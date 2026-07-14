//! The server (SSR) transform: parsed component → server-module JS + scoped CSS.
//!
//! Mirrors the canonical Svelte compiler's server output shape (the oracle):
//!
//! ```text
//! import * as $ from 'svelte/internal/server';
//! export default function Input($$renderer[, $$props]) {
//!     …instance script statements (rune-rewritten)…
//!     $$renderer.push(`…static html${$.escape(expr)}…`);
//! }
//! ```
//!
//! Static template emission follows the oracle's normalization, derived from
//! Svelte's own `clean_nodes` + `escape_html` (empirically probe-verified):
//!
//! - **Whitespace** (per fragment, whitespace class `[ \t\r\n]`): drop
//!   whitespace-only boundary text nodes and trim the boundary runs of edge
//!   text; collapse a text node's leading/trailing run to one space where it
//!   abuts a non-text node — except next to `{expr}` tags, which count as part
//!   of the text; keep interior whitespace verbatim. Inside `<pre>`/`<textarea>`
//!   nothing is normalized (a lone leading `"\n"` text in `<pre>` is dropped);
//!   inside `select`/`table`-family parents a collapsed space-only text is
//!   removed entirely. A component fragment starting with text/`{expr}` is
//!   prefixed with `<!---->`.
//! - **Entities**: text emits the *decoded* data re-escaped with `[&<]`
//!   (`&`→`&amp;`, `<`→`&lt;`); static attribute values re-escape with `[&"<]`.
//! - **Attributes**: a boolean attribute emits `name=""`; `class`/`style`
//!   values collapse `[ \t\n\r\f]+` runs to one space and trim; names emit
//!   lowercased; dynamic values become `$.attr(name, expr[, true])` /
//!   `$.attr_class($.clsx(expr))` / `$.attr_style(expr)`, and mixed text+expr
//!   values become attribute template literals with `$.stringify(expr)`
//!   interpolations (omitted when the oracle's evaluator proves a defined
//!   string).
//! - **Void elements** close with `/>`.
//! - **Statically-known template expressions fold** into the emitted text —
//!   the `analyze` evaluator ports the oracle's fold decision and refuses
//!   (`Gray`) anything it cannot bound byte-exactly.
//! - **Runes**: `$state(v)`/`$state.raw(v)` inits drop the wrapper (`void 0`
//!   when argument-less); `$derived(e)` → `$.derived(() => e)`;
//!   `$derived.by(f)` → `$.derived(f)`; a derived binding read as a bare
//!   template expression becomes `d()`; statement-position
//!   `$effect(…)`/`$effect.pre(…)` are dropped and force the
//!   `$$renderer.component(($$renderer) => { … })` wrapper.
//! - **`{@html expr}`** → a `${$.html(expr)}` interpolation (unescaped).
//!
//! Codegen owns zero precedence knowledge — the printer's `needs_parens`
//! handles it. Shapes the transform does not yet cover return a clear
//! [`CompileError::Unsupported`] rather than guessing.

use std::collections::BTreeSet;

use bumpalo::collections::Vec as BumpVec;
use tsv_css::ast::internal::{CssBlockChild, CssNode, SimpleSelector};
use tsv_lang::{InfallibleResolve, Span};
use tsv_svelte::ast::internal::{
    Attribute, AttributeNode, AttributeValue, AwaitBlock, ConstTag, EachBlock, Element,
    ElementKind, ExpressionTag, Fragment, FragmentNode, HtmlTag, IfBlock, KeyBlock, RenderTag,
    Root, SnippetBlock, SpecialElement, SpecialElementKind, Style,
};
use tsv_ts::ast::internal::{
    ArrayExpression, BinaryOperator, BlockStatement, ExportDefaultDeclaration, ExportDefaultValue,
    Expression, ExpressionStatement, ForInit, ForStatement, FunctionDeclaration, IfStatement,
    ImportDeclaration, ImportSpecifier, LiteralValue, ModuleExportName, ObjectExpression,
    ObjectPattern, ObjectPatternProperty, ObjectProperty, Property, PropertyKind, RestElement,
    Statement, UpdateOperator, VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
};

use std::collections::HashMap;

use crate::analyze::{
    Binding, BindingKind, Bindings, Initial, NameSet, RuneInit, Scope, ScopeEntry,
    classify_rune_init, evaluate, is_effect_call, pattern_binding_names, stringify_value,
};
use crate::attr_refs::{TemplateItem, each_template_item};
use crate::build::{Builder, escape_template_text};
use crate::rune_guard::{WalkCtx, walk_expression_guarded, walk_statement_guarded};
use crate::snippet::{SnippetAnalysis, snippet_name};
use crate::{CompileError, CompileOutput, Refusal, erase};

/// The deterministic scoping class — the fixed `cssHash` the oracle sidecar
/// compiles with, so outputs are byte-comparable across runs.
const SCOPE_HASH_CLASS: &str = "svelte-tsvhash";

/// The component function name. Derived from the constant filename the
/// deterministic oracle compiles under (`input.svelte` → `Input`).
const COMPONENT_NAME: &str = "Input";

/// Parents whose whitespace-only children are removed entirely instead of
/// collapsing to a single space (Svelte's `can_remove_entirely` list).
const REMOVE_WS_ENTIRELY_PARENTS: &[&str] = &[
    "select", "tr", "table", "tbody", "thead", "tfoot", "colgroup", "datalist",
];

/// Elements that emit `load`/`error` events (Svelte's `LOAD_ERROR_ELEMENTS`,
/// `utils.js`): an `onload`/`onerror` handler on one of these injects an
/// `on{name}="this.__e=event"` capture attribute instead of being dropped.
const LOAD_ERROR_ELEMENTS: &[&str] = &[
    "body", "embed", "iframe", "img", "link", "object", "script", "style", "track",
];

/// Whether `name` is a load-error element (see [`LOAD_ERROR_ELEMENTS`]).
fn is_load_error_element(name: &str) -> bool {
    LOAD_ERROR_ELEMENTS.contains(&name)
}

/// The DOM boolean attributes (the oracle's `DOM_BOOLEAN_ATTRIBUTES`): a
/// dynamic value on one of these emits `$.attr(name, value, true)`.
// TODO: consider a home in tsv_html beside the element classification tables.
const DOM_BOOLEAN_ATTRIBUTES: &[&str] = &[
    "allowfullscreen",
    "async",
    "autofocus",
    "autoplay",
    "checked",
    "controls",
    "default",
    "disabled",
    "formnovalidate",
    "indeterminate",
    "inert",
    "ismap",
    "loop",
    "multiple",
    "muted",
    "nomodule",
    "novalidate",
    "open",
    "playsinline",
    "readonly",
    "required",
    "reversed",
    "seamless",
    "selected",
    "webkitdirectory",
    "defer",
    "disablepictureinpicture",
    "disableremoteplayback",
];

/// Everything the emitters share: the builder, the script analysis products,
/// and the CSS scoping state.
struct EmitEnv<'arena, 's> {
    b: Builder<'arena>,
    source: &'s str,
    bindings: Bindings<'arena>,
    derived_names: NameSet,
    /// Top-level `$state`/`$state.raw` binding names. Stored as `Normal` in the
    /// binding table (a plain server variable after rewrite), but the oracle's
    /// `binding.kind` is `state` (non-`normal`), so a component whose name is a
    /// `$state` binding is dynamic — this set recovers that distinction.
    state_names: NameSet,
    scope: Option<ScopeInfo>,
    matched_classes: BTreeSet<String>,
    /// Script comments are being carried — emitters whose synthetic call
    /// windows would sweep host comments (`$.attr` family) must refuse.
    has_comments: bool,
    /// Active block-scope overlays (each items/indexes, `{:then}` values,
    /// `{@const}` bindings), innermost last.
    overlays: Vec<HashMap<String, ScopeEntry<'arena>>>,
    /// Inside an `{#each}` body — a nested each would need the oracle's
    /// unique-name allocation order, which is not confidently reproducible.
    in_each: bool,
    /// Source-order counters for the oracle's per-each unique names
    /// (`each_array`/`each_array_1`, `$$index`/`$$index_1`), advanced once per
    /// each block regardless of an authored index.
    each_array_count: usize,
    index_count: usize,
    /// The snippet hoist analysis: which top-level snippets go to module scope,
    /// and every snippet name (render-callee classification, name collisions).
    snippets: SnippetAnalysis,
    /// Module-scope `function` declarations for hoistable top-level snippets,
    /// emitted between the imports and the exported component function (source
    /// order — a top-level snippet is visited in order as the root fragment is
    /// walked).
    hoisted_snippets: Vec<Statement<'arena>>,
    /// Comment-refusal windows of the regions erased from **template** borrow
    /// points (the script's are collected before emission). See
    /// [`EmitEnv::erase`].
    erased_windows: Vec<Span>,
}

impl<'arena> EmitEnv<'arena, '_> {
    fn value_scope(&self) -> Scope<'_, 'arena> {
        Scope {
            bindings: &self.bindings,
            overlays: &self.overlays,
        }
    }

    /// Erase TypeScript from a borrowed **template** expression — the template's
    /// half of the oracle's phase-1 `remove_typescript_nodes`.
    ///
    /// Erasure applies per-expression **at the borrow point**; the Svelte AST is
    /// never rebuilt (every TypeScript-bearing markup position is a `tsv_ts`
    /// `Expression` reached through one of a small set of borrows). The erased
    /// node is what every consumer downstream of the borrow must see — not just
    /// the emitted argument, but the **static-evaluation fold gate beside it**
    /// (`x as T` would otherwise evaluate to UNKNOWN where the oracle folds `x`
    /// — a silent under-fold, a parity divergence no refusal catches) and the
    /// shape predicates (`class_needs_clsx`, the inline-string-literal check, the
    /// `{ n }` shorthand test) that read the node's variant. So the borrow point
    /// erases *once*, shadowing the raw node, and hands the erased one to all of
    /// them.
    ///
    /// A missed borrow point cannot ship TypeScript silently: the finished
    /// program is re-erased ([`self_check_no_typescript`]) and any survivor is a
    /// loud [`CompileError::TypeErasureLeak`].
    fn erase(
        &mut self,
        expr: &'arena Expression<'arena>,
    ) -> Result<&'arena Expression<'arena>, CompileError> {
        let arena = self.b.arena;
        let erased = erase::erase_expression(arena, self.source, expr)?;
        self.erased_windows.extend(erased.regions);
        Ok(match erased.expr {
            Some(new) => arena.alloc(new),
            None => expr,
        })
    }

    /// Push a block-scope overlay; refuses names that shadow a derived binding
    /// (the guard's derived-read refusal is name-based and can't see scopes).
    fn push_overlay(
        &mut self,
        entries: HashMap<String, ScopeEntry<'arena>>,
    ) -> Result<(), CompileError> {
        for name in entries.keys() {
            if self.derived_names.contains(name) {
                return Err(unsupported(Refusal::BlockScopeShadowsDerived {
                    name: name.clone(),
                }));
            }
        }
        self.overlays.push(entries);
        Ok(())
    }

    fn pop_overlay(&mut self) {
        self.overlays.pop();
    }

    /// The `each_array` unique name for the next `{#each}` (source order).
    fn next_each_array_name(&mut self) -> String {
        let n = self.each_array_count;
        self.each_array_count += 1;
        if n == 0 {
            "each_array".to_string()
        } else {
            format!("each_array_{n}")
        }
    }

    /// The `$$index` unique name for the next index-less `{#each}`.
    fn next_index_name(&mut self) -> String {
        let n = self.index_count;
        self.index_count += 1;
        if n == 0 {
            "$$index".to_string()
        } else {
            format!("$$index_{n}")
        }
    }
}

/// Compile a parsed component to server output.
pub(crate) fn compile_server<'arena>(
    root: &Root<'arena>,
    source: &str,
    arena: &'arena bumpalo::Bump,
) -> Result<CompileOutput, CompileError> {
    let mut b = Builder::new(arena, source, std::rc::Rc::clone(&root.interner));

    if root.module.is_some() {
        return Err(unsupported(Refusal::ModuleScript));
    }
    if root.options.is_some() {
        return Err(unsupported(Refusal::SvelteOptions));
    }
    // TypeScript erasure — the oracle's phase-1 `remove_typescript_nodes`, run
    // BEFORE every analysis pass (`analyze_script`, `analyze_snippets`,
    // `needs_context`) and before the codegen loop. `instance_body` is the
    // type-free statement list from here on; NOTHING below may read
    // `root.instance.content.body` (the un-erased tree still carries TS).
    let ts_document = document_ts_flag(root, source)?;
    let (instance_body, erased_windows) = match root.instance {
        Some(script) => {
            let erased = erase::erase_statements(arena, source, script.content.body)?;
            // The oracle gates the TypeScript *grammar* on the document-wide
            // `ts` flag: without it, a `: T` / `as T` / `x!` is a plain-JS parse
            // error. tsv's parser is TS-permissive and would silently accept —
            // an over-acceptance the refusal contract forbids. (The eraser also
            // unwraps `JsdocCast`, which is valid JavaScript, so the gate reads
            // `typescript`, not `changed`.)
            if erased.typescript && !ts_document {
                return Err(unsupported(Refusal::TypeScriptWithoutLangTs));
            }
            (erased.body, erased.regions)
        }
        None => (&[][..], Vec::new()),
    };
    // The template half of the same document-wide gate. The borrow points
    // (`EmitEnv::erase`) already erase every template expression that reaches
    // output, so this sweep exists for the ones that DON'T — see
    // `refuse_template_typescript`. It runs only without the flag.
    if !ts_document {
        refuse_template_typescript(root, source, arena)?;
    }

    // CSS scoping analysis (no minting): which class names are scoped, and
    // where the hash class splices into the style text.
    let scope = match root.css {
        Some(style) => Some(analyze_style(style, source)?),
        None => None,
    };

    // 1. `import * as $ from 'svelte/internal/server';`
    let import = b.import_namespace("$", "svelte/internal/server");

    // 2. Comment carry-through: script comments thread into the synthetic
    // program (their spans are host-absolute, so the detached-comment machinery
    // works against the buffer). Classes whose placement can't be made to
    // converge refuse — see `collect_script_comments`.
    let script_comments = collect_script_comments(root, source, instance_body)?;
    let has_comments = !script_comments.is_empty();
    // Comments alongside template blocks refuse: a block splits the template
    // into multiple pushes and moves content into branch bodies, and the
    // resulting comment-window placement is unprobed — refuse rather than risk a
    // misplaced comment.
    if has_comments && fragment_contains_block(&root.fragment) {
        return Err(unsupported(Refusal::CommentsAlongsideTemplateBlocks));
    }
    // Comments alongside a component invocation refuse: the component call's
    // minted object-literal / borrowed prop-value spans interleave with the
    // carried-comment windows in unprobed ways.
    if has_comments && fragment_has_component(&root.fragment) {
        return Err(unsupported(Refusal::CommentsWithComponent));
    }

    // 3. Script analysis pass: the top-level binding table (evaluator input)
    // and the derived-name set (read rewriting / refusal).
    let mut bindings = Bindings::empty();
    let mut derived_names = NameSet::default();
    analyze_script(instance_body, source, &mut bindings, &mut derived_names)?;
    if has_comments && !derived_names.is_empty() {
        // The `$.derived(() => …)` wrapper and `d()` reads bridge host and
        // appendix spans in ways whose comment windows sweep wrongly.
        return Err(unsupported(Refusal::CommentsWithDerived));
    }

    // 3b. Snippet hoist analysis: which top-level `{#snippet}`s go to module
    // scope. Imports don't disqualify hoisting, so the instance-binding set the
    // analysis subtracts is the binding table minus the import locals.
    let import_names: NameSet = instance_body
        .iter()
        .filter_map(|stmt| match stmt {
            Statement::ImportDeclaration(import) => Some(import),
            _ => None,
        })
        .flat_map(|import| import.specifiers)
        .filter_map(|spec| {
            use tsv_ts::ast::internal::ImportSpecifier;
            let local = match spec {
                ImportSpecifier::Default(s) => &s.local,
                ImportSpecifier::Named(s) => &s.local,
                ImportSpecifier::Namespace(s) => &s.local,
            };
            plain_identifier_name(local, source)
        })
        .collect();
    let instance_binding_names: NameSet = bindings.names().map(str::to_string).collect();
    let snippets =
        crate::snippet::analyze_snippets(root, source, &instance_binding_names, &import_names)?;
    // Script comments plus snippets/render reshape the component body (a hoisted
    // function, a per-render flush) in ways whose comment windows aren't probed;
    // refuse the combination.
    if has_comments && fragment_has_snippet_or_render(&root.fragment) {
        return Err(unsupported(Refusal::CommentsAlongsideTemplateBlocks));
    }

    // 4. Script rewrite pass: rune rewrites, guard walks, mutation/shadow
    // collection, effect detection. Rewrites drop source regions (rune call
    // wrappers, whole effect statements) — a comment inside a dropped region
    // has nowhere to go, so it refuses.
    let mut body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    // Instance-script `import` statements hoist to module scope (an `import`
    // inside the component function is invalid JS); the rest stay in the body.
    let mut user_imports: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    let mut uses_props = false;
    let mut has_effects = false;
    let mut updated = NameSet::default();
    let mut nested_declared = NameSet::default();
    let mut dropped_regions: Vec<Span> = Vec::new();
    // The whole-component analysis runs up front because the script rewrite
    // needs `uses_slots` (the `$props()` rest injection renames its destructured
    // `$$slots` to `$$slots_` when the injected sanitize_slots binding exists —
    // a duplicate `$$slots` lexical declaration would be invalid JS). Its error
    // is NOT propagated here: the script-loop refusals below must keep winning
    // for inputs that trip both, so the `?` stays at the original position.
    let component = crate::needs_context::analyze_component(root, source, instance_body);
    let uses_slots = component.as_ref().is_ok_and(|c| c.uses_slots);
    for stmt in instance_body {
        // Instance-script exports refuse — every form. The oracle compiles
        // `export const`/`function`/`{a}` into a trailing
        // `$.bind_props($$props, { … })` (not implemented), rejects
        // `export default` and `export let` (runes mode), and drops
        // `export * from`; passing any of them through verbatim would nest
        // an `export` inside the component function (invalid JS).
        if matches!(
            stmt,
            Statement::ExportNamedDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::TSNamespaceExportDeclaration(_)
                | Statement::TSExportAssignment(_)
        ) {
            return Err(unsupported(Refusal::InstanceScriptExport));
        }
        if let Statement::ImportDeclaration(import) = stmt {
            refuse_runes_invalid_import(import, source)?;
            user_imports.push(stmt.clone());
            continue;
        }
        let rewritten = rewrite_script_statement(
            &mut b,
            stmt,
            source,
            &derived_names,
            &mut updated,
            &mut nested_declared,
            &mut uses_props,
            &mut has_effects,
            has_comments,
            uses_slots,
            &mut dropped_regions,
        )?;
        let Some(rewritten) = rewritten else {
            continue;
        };
        // The oracle splits a multi-declarator top-level declaration into
        // one declaration per declarator, source order (probe-verified for
        // let/const/var, no-init, dependent, destructured, and mixed
        // rune+plain declarators; the per-declarator rune rewrites above
        // compose with the split). Declarations in nested scopes (function
        // bodies, blocks, for-heads) stay joined — borrowed passthrough
        // already matches that. Comments refuse: the oracle re-anchors a
        // comment *inside* the split (`let // c` then the declarator on the
        // next line), a placement this transform can't reproduce.
        match rewritten {
            Statement::VariableDeclaration(decl) if decl.declarations.len() > 1 => {
                if has_comments {
                    return Err(unsupported(Refusal::CommentsAlongsideMultiDeclarator));
                }
                for declarator in decl.declarations {
                    body.push(Statement::VariableDeclaration(VariableDeclaration {
                        kind: decl.kind,
                        declarations: std::slice::from_ref(declarator),
                        declare: decl.declare,
                        span: declarator.span,
                    }));
                }
            }
            other => body.push(other),
        }
    }
    // A comment sitting among hoisted imports would land in the export program's
    // comment list while its import node prints in a separate module-scope
    // program — a placement this transform doesn't reconcile yet.
    if has_comments && !user_imports.is_empty() {
        return Err(unsupported(Refusal::CommentsAlongsideImports));
    }
    // The oracle wraps the whole body in `$$renderer.component(($$renderer) => …)`
    // whenever its `needs_context` analysis fires (a `new` expression or a
    // member/call rooted in a prop/import — see `needs_context`). A dropped
    // `$effect` is one such trigger (already modeled by `has_effects`); the port
    // covers the rest. `needs_context` also forces the `$$props` parameter (the
    // oracle's `should_inject_props` includes `should_inject_context`). The same
    // walk collects component-wide reassignments — including mutations inside
    // dropped event handlers — so a mutated binding is not statically folded.
    // (Computed above the script loop for `uses_slots`; its error surfaces here.)
    let component = component?;
    for name in &component.reassigned {
        updated.insert(name.clone());
    }
    for name in &updated {
        bindings.mark_updated(name);
    }
    for name in &nested_declared {
        bindings.mark_opaque(name);
    }
    // Names declared inside function-like subtrees anywhere in the component
    // (template event handlers included — the script side's `nested_declared`
    // already covers the script). A same-named component binding goes Opaque:
    // an assignment target inside such a subtree may resolve to the shadowing
    // local, so neither folding nor escaping the outer binding is provable —
    // reads refuse instead (the script side's exact envelope).
    for name in &component.fn_declared {
        bindings.mark_opaque(name);
    }

    let needs_context = has_effects || component.needs_context;
    if needs_context {
        uses_props = true;
    }
    // A `$$slots` reference makes the component inject
    // `const $$slots = $.sanitize_slots($$props)` (below) and take `$$props`
    // (the oracle's `should_inject_props` includes `uses_slots`). Carried script
    // comments plus the injected first statement would sweep the function-body
    // comment windows, so refuse that combination for now.
    if component.uses_slots {
        if has_comments {
            return Err(unsupported(Refusal::CommentsWithSlots));
        }
        uses_props = true;
    }
    for comment in &script_comments {
        for region in &dropped_regions {
            if comment.span.start >= region.start && comment.span.end <= region.end {
                return Err(unsupported(Refusal::CommentInRewrittenRuneRegion));
            }
        }
        // Type erasure deletes source regions too, and the oracle's
        // surviving-comment placement there is emergent (its printer claims
        // comments at scattered flush points using pre-erasure spans) — not a
        // rule this transform can port. A comment *intersecting* an erased
        // region's window refuses. The window runs past the erased span to the
        // next surviving token, so `let x: Foo /* c */ = v` — which the oracle
        // re-anchors onto the initializer — is caught too.
        for window in &erased_windows {
            if comment.span.start < window.end && comment.span.end > window.start {
                return Err(unsupported(Refusal::CommentInErasedTypeRegion));
            }
        }
    }

    // Top-level `$state`/`$state.raw` binding names — a component named after one
    // is dynamic (see `component_dynamic`).
    let mut state_names = NameSet::default();
    for stmt in instance_body {
        if let Statement::VariableDeclaration(decl) = stmt {
            for declarator in decl.declarations {
                if matches!(
                    declarator
                        .init
                        .as_ref()
                        .and_then(|init| classify_rune_init(init, source)),
                    Some(RuneInit::State(_))
                ) && let Some(name) = identifier_binding_name(&declarator.id, source)
                {
                    state_names.insert(name);
                }
            }
        }
    }

    let mut env = EmitEnv {
        b,
        source,
        bindings,
        derived_names,
        state_names,
        scope,
        matched_classes: BTreeSet::new(),
        has_comments,
        overlays: Vec::new(),
        in_each: false,
        each_array_count: 0,
        index_count: 0,
        snippets,
        hoisted_snippets: Vec::new(),
        erased_windows: Vec::new(),
    };

    // 5. Function header skeleton. The text is minted for appendix
    // readability, but the header NODES carry fictional zero spans: every
    // comment window they anchor is then empty or inverted (both yield no
    // comments), so host-span script comments are only ever found by the
    // function-body block windows — the one place they belong. The body block
    // anchors on the script content start so leading script comments emit
    // before the first statement.
    env.b.mint("export default function ");
    let fn_id = env.b.ident_at(COMPONENT_NAME, Span::new(0, 0));
    env.b.mint("(");
    let params_start = 0;
    let mut params: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    let renderer = env.b.ident_at("$$renderer", Span::new(0, 0));
    params.push(Expression::Identifier(renderer));
    if uses_props {
        env.b.mint(", ");
        let props = env.b.ident_at("$$props", Span::new(0, 0));
        params.push(Expression::Identifier(props));
    }
    let lbrace = env.b.mint(") {").end - 1;
    let block_start = root
        .instance
        .map_or(lbrace, |script| script.content.span.start);

    // 6. Emit the template into the body: statements interleaved with
    // `$$renderer.push(…)` template flushes (blocks split the pushes).
    let mut out = BodyBuilder::new_in(arena);
    for stmt in body {
        out.stmts.push(stmt);
    }
    emit_fragment(
        &mut env,
        &root.fragment,
        &mut out,
        FragmentCtx {
            mark_text_first: true,
            is_component_root: true,
            hoist_snippets: true,
            is_standalone: false,
            preserve_whitespace: false,
            parent_name: None,
        },
    )?;
    let body = out.finish(&mut env.b, arena);

    // The template's erased regions get the script's comment rule (a comment in
    // an erased region's window refuses — the oracle's surviving-comment
    // placement there is emergent, not portable). No carried comment can reach a
    // template window today: `collect_script_comments` refuses *any* comment
    // outside the instance script, and refuses a fragment node that starts before
    // the script ends — so the two ranges are disjoint by construction. The check
    // stays because the rule, not the geometry, is the contract.
    for comment in &script_comments {
        for window in &env.erased_windows {
            if comment.span.start < window.end && comment.span.end > window.start {
                return Err(unsupported(Refusal::CommentInErasedTypeRegion));
            }
        }
    }

    // A scoped selector that matches no element would be pruned by the oracle —
    // pruning isn't implemented, so refuse rather than emit unpruned CSS.
    if let Some(scope) = &env.scope {
        for class in &scope.class_names {
            if !env.matched_classes.contains(class) {
                return Err(unsupported(Refusal::CssSelectorNoMatch {
                    class: class.clone(),
                }));
            }
        }
    }

    // 7. `needs_context` (a dropped effect, or the ported new/member/call
    // analysis) forces the `$$renderer.component(($$renderer) => { … })` wrapper
    // around the whole body.
    let body = if needs_context {
        let inner_span = Span::new(block_start, env.b.buffer.len() as u32);
        let wrapper_renderer = env.b.ident("$$renderer");
        let mut wrapper_params: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
        wrapper_params.push(Expression::Identifier(wrapper_renderer));
        let arrow = env
            .b
            .arrow_block(wrapper_params.into_bump_slice(), body, inner_span);
        let arrow_alloc = env.b.arena.alloc(arrow);
        let call = env
            .b
            .member_call("$$renderer", "component", std::slice::from_ref(arrow_alloc));
        let mut outer: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        // A FICTIONAL span, low start + appendix end — not the call's own span.
        //
        // The wrapper is the function body's only statement, so the printer's
        // comment windows for it are `(function_block.start … this.start)` before
        // and `(this.end … function_block.end)` after. The call's real span starts
        // in the appendix (past every host comment), so the LEADING window would
        // cover the whole script and sweep every carried comment — which the
        // arrow's own block, anchored on the same script start, then sweeps
        // AGAIN. The comment prints twice.
        //
        // A zero start inverts the leading window to empty; an appendix end keeps
        // the trailing window inside the appendix, where no host comment lives.
        // The arrow's block is then the sole owner — the oracle's placement,
        // inside the wrapper.
        outer.push(Statement::ExpressionStatement(ExpressionStatement {
            expression: call,
            span: Span::new(0, env.b.buffer.len() as u32),
            is_directive: false,
        }));
        outer.into_bump_slice()
    } else {
        body
    };

    // The oracle unshifts `const $$slots = $.sanitize_slots($$props)` to the top
    // of the component function body — before the wrapper (`transform-server.js`
    // `:300`). Prepend it here so it sits outside any `$$renderer.component`
    // wrapper, matching the oracle's placement.
    let body = if component.uses_slots {
        let slots_decl = build_sanitize_slots_decl(&mut env.b, arena);
        let mut with_slots: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        with_slots.push(slots_decl);
        with_slots.extend_from_slice(body);
        with_slots.into_bump_slice()
    } else {
        body
    };

    // 8. Assemble and print. The import and the component function print as
    // TWO single-statement programs, concatenated: the import's source literal
    // must keep a real appendix span (its text is extracted), and any window
    // from a low anchor to that appendix span would sweep every host-span
    // comment — a separate comment-free program sidesteps the whole class.
    // Concatenation equals the single-program print: canonical mode joins
    // top-level statements with exactly one hardline and no blank lines.
    let rbrace_end = env.b.mint("}").end;
    let function = FunctionDeclaration {
        id: Some(fn_id),
        type_parameters: None,
        params: params.into_bump_slice(),
        return_type: None,
        body: BlockStatement {
            body,
            span: Span::new(block_start, rbrace_end),
        },
        generator: false,
        r#async: false,
        params_start,
        span: Span::new(0, rbrace_end),
    };
    let export = ExportDefaultDeclaration {
        declaration: ExportDefaultValue::FunctionDeclaration(function),
        span: Span::new(0, rbrace_end),
    };

    // The module scaffold: the `$` runtime import, then the user's hoisted
    // instance-script imports in source order. All are comment-free, so no
    // low-anchor→appendix window can sweep a host comment (the reason the import
    // and export programs stay separate).
    let mut import_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    import_body.push(Statement::ImportDeclaration(import));
    for user_import in user_imports {
        import_body.push(user_import);
    }
    let import_program = tsv_ts::ast::internal::Program {
        body: import_body.into_bump_slice(),
        comments: Vec::new(),
        span: Span::new(0, env.b.buffer.len() as u32),
        interner: std::rc::Rc::clone(&root.interner),
        goal: tsv_ts::Goal::Module,
    };

    // Hoistable top-level snippets print as their own comment-free program,
    // between the imports and the exported component function (the oracle's
    // module-scope placement). Empty when nothing hoists.
    let hoisted_program = (!env.hoisted_snippets.is_empty()).then(|| {
        let mut hoisted_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        for snippet in env.hoisted_snippets.drain(..) {
            hoisted_body.push(snippet);
        }
        tsv_ts::ast::internal::Program {
            body: hoisted_body.into_bump_slice(),
            comments: Vec::new(),
            span: Span::new(0, env.b.buffer.len() as u32),
            interner: std::rc::Rc::clone(&root.interner),
            goal: tsv_ts::Goal::Module,
        }
    });

    let mut export_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    export_body.push(Statement::ExportDefaultDeclaration(export));
    let export_program = tsv_ts::ast::internal::Program {
        body: export_body.into_bump_slice(),
        comments: script_comments,
        span: Span::new(0, env.b.buffer.len() as u32),
        interner: std::rc::Rc::clone(&root.interner),
        goal: tsv_ts::Goal::Module,
    };

    // 8b. The mandatory type-erasure self-check, run on the FINISHED program.
    //
    // `compile`'s output-reparse validation cannot catch a missed erase: tsv's
    // parser is TypeScript-permissive, so a surviving annotation still parses,
    // flows through the pipeline untouched, and prints verbatim — a silent
    // mis-compile. Re-running the eraser is the check: by its `None`-means-
    // unchanged contract, `changed == false` PROVES the program holds no
    // TypeScript-only node. One walk, and the inventory it checks against is the
    // same one that did the erasing — nothing to drift.
    self_check_no_typescript(
        arena,
        &env.b.buffer,
        &[
            import_program.body,
            hoisted_program.as_ref().map_or(&[][..], |p| p.body),
            export_program.body,
        ],
    )?;

    let mut js = tsv_ts::format_canonical(&import_program, &env.b.buffer);
    if let Some(hoisted_program) = &hoisted_program {
        js.push_str(&tsv_ts::format_canonical(hoisted_program, &env.b.buffer));
    }
    js.push_str(&tsv_ts::format_canonical(&export_program, &env.b.buffer));
    let css = match (root.css, &env.scope) {
        (Some(style), Some(scope)) => Some(splice_scoped_css(style, source, scope)),
        _ => None,
    };

    Ok(CompileOutput {
        js,
        css,
        warnings: Vec::new(),
    })
}

/// Collect the comments carried into the synthetic program: exactly the host
/// comments inside the instance script's content span. Classes that can't
/// converge refuse:
///
/// - comments outside the script (template-expression comments) — the emitters
///   don't thread them yet;
/// - a fragment node *before* the script end (template-before-script) — the
///   `$.escape`/`$.html` wrapper windows would sweep script comments;
/// - format-ignore directives — they'd switch the printer to raw-source
///   emission of synthetic spans.
fn collect_script_comments(
    root: &Root<'_>,
    source: &str,
    instance_body: &[Statement<'_>],
) -> Result<Vec<tsv_lang::Comment>, CompileError> {
    if root.comments.is_empty() {
        return Ok(Vec::new());
    }
    let Some(script) = root.instance else {
        return Err(unsupported(Refusal::TemplateComments));
    };
    let content = script.content.span;
    // The statement bounds are read from the ERASED body: an erased statement
    // prints nothing, so the printer's first/last emitted statement is the first
    // and last *surviving* one.
    //
    // A comment after the LAST script statement diverges: the oracle's printer
    // re-attaches it as a leading comment of the next emitted node (inside the
    // template's `$.escape(…)` argument), a placement this transform can't
    // reproduce — refuse the class.
    let last_stmt_end = instance_body
        .last()
        .map_or(content.start, |stmt| stmt.span().end);
    // A leading comment glued to the `<script>` line (no newline before it) shares
    // its source line with the function's synthetic opening brace, so the printer
    // trails it after the `{` instead of onto its own line — refuse the class
    // (prettier-formatted input always puts a leading comment on its own line, so
    // the covered fixtures are unaffected).
    let first_stmt_start = instance_body
        .first()
        .map_or(content.end, |stmt| stmt.span().start);
    let mut comments = Vec::with_capacity(root.comments.len());
    for comment in &root.comments {
        if comment.span.start < content.start || comment.span.end > content.end {
            return Err(unsupported(Refusal::TemplateComments));
        }
        if comment.span.start >= last_stmt_end {
            return Err(unsupported(Refusal::CommentAfterLastStatement));
        }
        if comment.span.end <= first_stmt_start {
            let gap = &source[content.start as usize..comment.span.start as usize];
            if !gap.contains('\n') {
                return Err(unsupported(Refusal::LeadingCommentGluedToScript));
            }
        }
        let text = comment.content(source);
        if text.contains("prettier-ignore") || text.contains("format-ignore") {
            return Err(unsupported(Refusal::FormatIgnoreComment));
        }
        let mut comment = comment.clone();
        // Release a JSDoc cast's comment back to the positional machinery. `tsv_ts`
        // binds it to its `JsdocCast` node (`Comment::owned_by_node`) so a synthesized
        // paren can't land between the comment and the `(` it glues to — the owning
        // node becomes the only thing that prints it, and the range lookups skip it.
        // Erasure unwraps *every* `JsdocCast` (the compile path matches the oracle,
        // which has no such node and drops the parens), so in the emitted program that
        // owner does not exist: left owned, the comment is printed by nothing and
        // silently dropped. Un-owned, it prints from its gap exactly as the oracle
        // prints it — `const x = /** @type {number} */ 1`.
        comment.owned_by_node = false;
        comments.push(comment);
    }
    for node in root.fragment.nodes {
        if node.span().start < content.end {
            return Err(unsupported(Refusal::CommentsWithTemplateBeforeScript));
        }
    }
    Ok(comments)
}

fn unsupported(reason: Refusal) -> CompileError {
    CompileError::Unsupported(reason)
}

/// The oracle's `unthunk` peephole, at the only arity a thunk can have.
///
/// `b.thunk(value)` builds `arrow([], value)` and immediately runs it through
/// `unthunk`, which returns the call's **callee** when the arrow is non-async,
/// its body is a `CallExpression` with an `Identifier` callee, and its
/// parameters match the call's arguments one-for-one by name
/// (`utils/builders.js`). A thunk's parameter list is always empty, so the
/// name-matching clause reduces to "the call takes no arguments":
///
/// - `$derived(get_library())` → `$.derived(get_library)`
/// - `$derived(f(a))` → `$.derived(() => f(a))` (an argument survives)
/// - `$derived(o.m())` → `$.derived(() => o.m())` (the callee is not an identifier)
///
/// An optional call (`f?.()`) is a `ChainExpression` in the oracle's AST, never a
/// bare `CallExpression`, so it never collapses either.
fn unthunk_callee<'arena>(expr: &Expression<'arena>) -> Option<&'arena Expression<'arena>> {
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    if !call.arguments.is_empty() || call.optional {
        return None;
    }
    matches!(call.callee, Expression::Identifier(_)).then_some(call.callee)
}

/// Assert no TypeScript-only node survived into the emitted program.
///
/// Both halves of the erasure — the instance script's `Program` and each
/// template expression at its borrow point — run before this, so **any**
/// survivor is a compiler bug: an erase case missed, or a borrow point that
/// never called [`EmitEnv::erase`]. It is surfaced loudly as
/// [`CompileError::TypeErasureLeak`] rather than emitted.
///
/// This is the check the output reparse cannot make: tsv's parser is
/// TypeScript-permissive, so a surviving annotation parses, flows through the
/// pipeline untouched, and prints verbatim. The eraser's `None`-means-unchanged
/// contract makes "no change" a *proof* of no TypeScript — and it is the same
/// inventory that did the erasing, so there is nothing to drift.
fn self_check_no_typescript<'arena>(
    arena: &'arena bumpalo::Bump,
    buffer: &str,
    programs: &[&'arena [Statement<'arena>]],
) -> Result<(), CompileError> {
    for body in programs {
        let checked = erase::erase_statements(arena, buffer, body)?;
        if checked.changed {
            let leak = checked
                .regions
                .first()
                .copied()
                .unwrap_or_else(|| Span::new(0, 0));
            return Err(CompileError::TypeErasureLeak(leak));
        }
    }
    Ok(())
}

/// The oracle's **document-wide** TypeScript flag.
///
/// Svelte's parser regexes the raw source for the *first* `<script>` carrying a
/// `lang` attribute and tests its value `=== 'ts'` **exactly** — case-sensitive,
/// so `lang="typescript"` and `lang="TS"` are NOT TypeScript (they become
/// plain-JS parse errors). That one flag then selects the TypeScript grammar for
/// **every** `<script>` *and* every template mustache, block pattern, and snippet
/// `<T>` clause. So the decision belongs to the document, not to a `<script>` tag.
///
/// A module `<script>` is refused before this runs, so the instance script is the
/// only lang-bearing script here. `generics` is refused outright (an open
/// type-parameter *binding*, not annotation erasure), as is any `lang` other than
/// `ts`/`js`/empty.
fn document_ts_flag(root: &Root<'_>, source: &str) -> Result<bool, CompileError> {
    let Some(script) = root.instance else {
        return Ok(false);
    };
    let mut ts = false;
    for attr_node in script.attributes {
        let AttributeNode::Attribute(attr) = attr_node else {
            continue;
        };
        let name = {
            let interner = script.content.interner.borrow();
            interner.resolve_infallible(attr.name).to_string()
        };
        match name.as_str() {
            "lang" => match attr.value {
                // A bare `lang` (no value) never matches the oracle's regex —
                // plain JS, like no attribute at all.
                Some([]) | None => {}
                Some([AttributeValue::Text(text)]) => {
                    let lang = text.data(source);
                    match lang.as_ref() {
                        "ts" => ts = true,
                        "js" | "" => {}
                        _ => {
                            return Err(unsupported(Refusal::LangInstanceScript {
                                lang: lang.into_owned(),
                            }));
                        }
                    }
                }
                // An expression-valued `lang` can't be classified.
                _ => {
                    return Err(unsupported(Refusal::LangInstanceScript {
                        lang: String::new(),
                    }));
                }
            },
            "generics" => {
                return Err(unsupported(Refusal::GenericsAttribute));
            }
            _ => {}
        }
    }
    Ok(ts)
}

/// The **template** half of the document-wide TypeScript gate: refuse any
/// TypeScript in the template of a component with no `lang="ts"`.
///
/// Without the flag the oracle's parser rejects TypeScript *anywhere* in the
/// document — every mustache, block pattern, and snippet `<T>` clause included
/// (see [`document_ts_flag`]). tsv's parser is TypeScript-permissive everywhere,
/// so the decision has to be made explicitly here or the component is an
/// over-acceptance.
///
/// The borrow points ([`EmitEnv::erase`]) already erase every template expression
/// that reaches **output**, so this sweep exists for the ones that do *not*: the
/// SSR-dropped `{#each}` key, the `{#key}` expression, the `{:catch}` binding and
/// its whole branch, and event-handler attributes. Their TypeScript never reaches
/// the emitted program, so the erase self-check cannot see it either.
///
/// The eraser stays the single TypeScript inventory — this never re-decides *what
/// is TypeScript*, it only routes every template item through
/// [`erase::erase_expression`] and refuses on its `typescript` flag. The traversal
/// is `attr_refs`'s shared, exhaustively-matched one, so a new template shape fails
/// compilation rather than slipping past. Runs only when the flag is absent, so the
/// ordinary TypeScript path pays nothing.
///
/// # Soundness precondition
///
/// **The sweep is sound only if `tsv_svelte`'s parser preserves every TypeScript
/// node it parses.** It reasons about TypeScript by walking the tree, so a node the
/// parser *drops* is a node it cannot see — and cannot refuse. That is not
/// hypothetical: the block-pattern readers once parsed a destructured binding's
/// `: T` and threw it away (no node, no span, no error), and this sweep let
/// `{#await p then { a }: { a: number }}` through in a document with no `lang="ts"`,
/// where the oracle parse-errors. A dropped node is an invisible node. The same
/// precondition backs the erase self-check, for the same reason.
fn refuse_template_typescript<'arena>(
    root: &Root<'arena>,
    source: &str,
    arena: &'arena bumpalo::Bump,
) -> Result<(), CompileError> {
    each_template_item(&root.fragment, &mut |item| {
        let typescript = match item {
            TemplateItem::Expression(expr) => {
                erase::erase_expression(arena, source, expr)?.typescript
            }
            TemplateItem::SnippetTypeParameters => true,
        };
        if typescript {
            return Err(unsupported(Refusal::TypeScriptWithoutLangTs));
        }
        Ok(())
    })
}

/// Analysis pass: populate the top-level binding table and the derived-name
/// set from the script's top-level declarations.
fn analyze_script<'arena>(
    stmts: &'arena [Statement<'arena>],
    source: &str,
    bindings: &mut Bindings<'arena>,
    derived_names: &mut NameSet,
) -> Result<(), CompileError> {
    for stmt in stmts {
        match stmt {
            Statement::VariableDeclaration(decl) => {
                for declarator in decl.declarations {
                    analyze_declarator(declarator, source, bindings, derived_names)?;
                }
            }
            Statement::FunctionDeclaration(f) => {
                if let Some(name) =
                    f.id.as_ref()
                        .and_then(|id| plain_identifier_name(id, source))
                {
                    bindings.insert(
                        name,
                        Binding {
                            kind: BindingKind::Normal,
                            initial: Initial::Function,
                            updated: false,
                        },
                    );
                }
            }
            Statement::ImportDeclaration(import) => {
                use tsv_ts::ast::internal::ImportSpecifier;
                for spec in import.specifiers {
                    let local = match spec {
                        ImportSpecifier::Default(s) => &s.local,
                        ImportSpecifier::Named(s) => &s.local,
                        ImportSpecifier::Namespace(s) => &s.local,
                    };
                    if let Some(name) = plain_identifier_name(local, source) {
                        bindings.insert(
                            name,
                            Binding {
                                kind: BindingKind::Normal,
                                initial: Initial::None,
                                updated: false,
                            },
                        );
                    }
                }
            }
            Statement::ClassDeclaration(class) => {
                if let Some(name) = class
                    .id
                    .as_ref()
                    .and_then(|id| plain_identifier_name(id, source))
                {
                    bindings.insert(
                        name,
                        Binding {
                            kind: BindingKind::Normal,
                            initial: Initial::None,
                            updated: false,
                        },
                    );
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn plain_identifier_name(
    id: &tsv_ts::ast::internal::Identifier<'_>,
    source: &str,
) -> Option<String> {
    if id.escaped_name.is_some() {
        return None;
    }
    let start = id.span.start as usize;
    Some(source[start..start + id.name_len as usize].to_string())
}

/// Mirror the oracle's runes-mode import rules (its analyze-phase
/// `ImportDeclaration` visitor): any `svelte/internal*` source is forbidden
/// (private runtime code), and `beforeUpdate`/`afterUpdate` cannot be
/// imported from `svelte`. A string-literal imported name is skipped exactly
/// as the oracle skips it (its check matches `Identifier` names only); an
/// escaped identifier imported from `svelte` refuses conservatively — the
/// oracle compares the DECODED name, which this raw-span read can't see.
fn refuse_runes_invalid_import(
    import: &ImportDeclaration<'_>,
    source: &str,
) -> Result<(), CompileError> {
    let LiteralValue::String(cooked) = &import.source.value else {
        return Ok(());
    };
    let specifier = cooked.resolve(import.source.span, source);
    if specifier.starts_with("svelte/internal") {
        return Err(unsupported(Refusal::SvelteInternalImport));
    }
    if specifier == "svelte" {
        for spec in import.specifiers {
            let ImportSpecifier::Named(named) = spec else {
                continue;
            };
            let ModuleExportName::Identifier(imported) = &named.imported else {
                continue;
            };
            match plain_identifier_name(imported, source) {
                Some(name) if name == "beforeUpdate" || name == "afterUpdate" => {
                    return Err(unsupported(Refusal::RunesInvalidImport { name }));
                }
                Some(_) => {}
                None => {
                    return Err(unsupported(Refusal::RunesInvalidImport {
                        name: "escaped identifier".to_string(),
                    }));
                }
            }
        }
    }
    Ok(())
}

/// Classify one top-level declarator into the binding table.
fn analyze_declarator<'arena>(
    declarator: &'arena VariableDeclarator<'arena>,
    source: &str,
    bindings: &mut Bindings<'arena>,
    derived_names: &mut NameSet,
) -> Result<(), CompileError> {
    let rune = declarator
        .init
        .as_ref()
        .and_then(|init| classify_rune_init(init, source));

    match rune {
        Some(RuneInit::Props) => {
            let mut names = Vec::new();
            pattern_binding_names(&declarator.id, source, &mut names)?;
            for name in names {
                bindings.insert(
                    name,
                    Binding {
                        kind: BindingKind::Prop,
                        initial: Initial::None,
                        updated: false,
                    },
                );
            }
            Ok(())
        }
        Some(RuneInit::State(arg)) => {
            let name = identifier_binding_name(&declarator.id, source)
                .ok_or_else(|| unsupported(Refusal::DestructuringState))?;
            bindings.insert(
                name,
                Binding {
                    kind: BindingKind::Normal,
                    initial: arg.map_or(Initial::Undefined, Initial::Expr),
                    updated: false,
                },
            );
            Ok(())
        }
        Some(RuneInit::Derived(expr)) => {
            let name = identifier_binding_name(&declarator.id, source)
                .ok_or_else(|| unsupported(Refusal::DestructuringDerived))?;
            derived_names.insert(name.clone());
            bindings.insert(
                name,
                Binding {
                    kind: BindingKind::Derived,
                    initial: Initial::Expr(expr),
                    updated: false,
                },
            );
            Ok(())
        }
        Some(RuneInit::DerivedBy(f)) => {
            let name = identifier_binding_name(&declarator.id, source)
                .ok_or_else(|| unsupported(Refusal::DestructuringDerivedBy))?;
            derived_names.insert(name.clone());
            // The oracle evaluates through an expression-bodied arrow.
            use tsv_ts::ast::internal::ArrowFunctionBody;
            let initial = match f {
                Expression::ArrowFunctionExpression(arrow) => match &arrow.body {
                    ArrowFunctionBody::Expression(body) => Initial::Expr(body),
                    ArrowFunctionBody::BlockStatement(_) => Initial::None,
                },
                _ => Initial::None,
            };
            bindings.insert(
                name,
                Binding {
                    kind: BindingKind::Derived,
                    initial,
                    updated: false,
                },
            );
            Ok(())
        }
        None => {
            // Plain declarator: an Identifier id gets its init as the
            // evaluation initial; destructured ids are Opaque (the oracle's
            // per-binding initial for those isn't modeled).
            if let Some(name) = identifier_binding_name(&declarator.id, source) {
                bindings.insert(
                    name,
                    Binding {
                        kind: BindingKind::Normal,
                        initial: declarator
                            .init
                            .as_ref()
                            .map_or(Initial::None, Initial::Expr),
                        updated: false,
                    },
                );
            } else {
                let mut names = Vec::new();
                pattern_binding_names(&declarator.id, source, &mut names)?;
                for name in names {
                    bindings.insert(
                        name,
                        Binding {
                            kind: BindingKind::Opaque,
                            initial: Initial::None,
                            updated: false,
                        },
                    );
                }
            }
            Ok(())
        }
    }
}

fn identifier_binding_name(id: &Expression<'_>, source: &str) -> Option<String> {
    let Expression::Identifier(ident) = id else {
        return None;
    };
    plain_identifier_name(ident, source)
}

/// Rewrite one instance-script statement for the server module:
///
/// - a top-level `$props()` declarator init becomes `$$props` (and the
///   component gains the `$$props` param);
/// - `$state(v)` / `$state.raw(v)` inits drop the wrapper (`void 0` when
///   argument-less);
/// - `$derived(e)` → `$.derived(() => e)`; `$derived.by(f)` → `$.derived(f)`;
/// - statement-position `$effect(…)` / `$effect.pre(…)` are dropped
///   (returning `None`) and force the component wrapper;
/// - everything else passes through borrowed after the guard walk (which also
///   collects mutations and shadow names for the evaluator).
///
/// Passthrough/rebuild is a *shallow* re-slot: `Statement`/`VariableDeclarator`
/// hold children inline by value, so placing a borrowed statement into the
/// synthetic body clones the wrapper only — children remain shared `&'arena`
/// refs into the parsed AST, and the original wrapper never enters the printed
/// tree (no duplicate spans in what the printer walks). See `build.rs` for the
/// address-keyed side-table caveat.
#[allow(clippy::too_many_arguments)]
fn rewrite_script_statement<'arena>(
    b: &mut Builder<'arena>,
    stmt: &'arena Statement<'arena>,
    source: &str,
    derived_names: &NameSet,
    updated: &mut NameSet,
    nested_declared: &mut NameSet,
    uses_props: &mut bool,
    has_effects: &mut bool,
    has_comments: bool,
    uses_slots: bool,
    dropped_regions: &mut Vec<Span>,
) -> Result<Option<Statement<'arena>>, CompileError> {
    // A top-level `$:` label is a legacy reactive statement — invalid in
    // runes mode (the oracle rejects it with legacy_reactive_statement_invalid),
    // so cloning it through would emit a dead label with no reactivity, a
    // silent mis-compile. Only the top level refuses: the oracle accepts a
    // `$` label inside a function (an ordinary JS label) and clones it
    // through, as does the fallback below. An escaped label name can't be
    // classified from its raw span, so it refuses conservatively.
    if let Statement::LabeledStatement(labeled) = stmt {
        let label = &labeled.label;
        let is_dollar = label.escaped_name.is_some() || {
            let start = label.span.start as usize;
            &source[start..start + label.name_len as usize] == "$"
        };
        if is_dollar {
            return Err(unsupported(Refusal::LegacyReactiveStatement));
        }
    }

    // Statement-position effects are dropped (and force the wrapper); their
    // callback is still guard-walked so stray runes inside refuse.
    if let Statement::ExpressionStatement(expr_stmt) = stmt
        && let Some(callback) = is_effect_call(&expr_stmt.expression, source)
    {
        *has_effects = true;
        dropped_regions.push(stmt.span());
        let mut ctx = WalkCtx::new(source, updated, nested_declared, derived_names);
        walk_expression_guarded(callback, &mut ctx)?;
        return Ok(None);
    }

    let Statement::VariableDeclaration(decl) = stmt else {
        let mut ctx = WalkCtx::new(source, updated, nested_declared, derived_names);
        walk_statement_guarded(stmt, &mut ctx, 0)?;
        return Ok(Some(stmt.clone()));
    };

    let has_rune_init = decl.declarations.iter().any(|d| {
        d.init
            .as_ref()
            .is_some_and(|i| classify_rune_init(i, source).is_some())
    });
    if !has_rune_init {
        let mut ctx = WalkCtx::new(source, updated, nested_declared, derived_names);
        walk_statement_guarded(stmt, &mut ctx, 0)?;
        return Ok(Some(stmt.clone()));
    }

    let mut declarations: BumpVec<'arena, VariableDeclarator<'arena>> = BumpVec::new_in(b.arena);
    for declarator in decl.declarations {
        let mut ctx = WalkCtx::new(source, updated, nested_declared, derived_names);
        let rune = declarator
            .init
            .as_ref()
            .and_then(|init| classify_rune_init(init, source));

        // Guard the binding pattern (a rune or derived read can hide in a
        // pattern default) — except for state/derived declarators, whose id is
        // an enforced plain identifier and is a *declaration* of the (possibly
        // derived) name, not a read.
        if matches!(rune, None | Some(RuneInit::Props)) {
            walk_expression_guarded(&declarator.id, &mut ctx)?;
        }
        // A rune init rewrite drops the call's own syntax around the kept
        // argument — record the dropped region(s) so comments inside refuse.
        if let (Some(init), Some(_)) = (&declarator.init, &rune) {
            let init_span = init.span();
            match rune {
                Some(RuneInit::State(Some(arg)))
                | Some(RuneInit::Derived(arg))
                | Some(RuneInit::DerivedBy(arg)) => {
                    let arg_span = arg.span();
                    dropped_regions.push(Span::new(init_span.start, arg_span.start));
                    dropped_regions.push(Span::new(arg_span.end, init_span.end));
                }
                _ => dropped_regions.push(init_span),
            }
        }

        let mut new_id = declarator.id.clone();
        let new_init = match rune {
            Some(RuneInit::Props) => {
                *uses_props = true;
                if let Some(injected) =
                    inject_props_pattern(b, &declarator.id, has_comments, uses_slots)?
                {
                    new_id = injected;
                }
                // Span-steal: the synthetic `$$props` takes the replaced
                // `$props()` call's host span, so the declarator's `=`-gap
                // comment windows stay exactly the authored ones.
                let init_span = declarator
                    .init
                    .as_ref()
                    .map_or(declarator.span, Expression::span);
                let props_ident = b.ident_at("$$props", init_span);
                Some(Expression::Identifier(props_ident))
            }
            Some(RuneInit::State(arg)) => match arg {
                Some(arg) => {
                    walk_expression_guarded(arg, &mut ctx)?;
                    Some(arg.clone())
                }
                None => {
                    if has_comments {
                        // `void 0` mints an appendix literal; the declarator's
                        // init windows would then sweep host comments.
                        return Err(unsupported(Refusal::CommentsWithArglessState));
                    }
                    Some(b.void_zero())
                }
            },
            Some(RuneInit::Derived(expr)) => {
                walk_expression_guarded(expr, &mut ctx)?;
                // The oracle wraps the value with `b.thunk`, which is
                // `unthunk(arrow([], value))` — and `unthunk` COLLAPSES the arrow
                // when its body is a plain call whose callee is a bare identifier
                // and whose arguments match the parameter list one-for-one by
                // name (`utils/builders.js`; call site
                // `3-transform/server/visitors/VariableDeclaration.js`). With the
                // empty parameter list a thunk always has, that reduces to an
                // argument-less, non-optional call on an identifier — so
                // `$derived(get_library())` emits `$.derived(get_library)`, not
                // `$.derived(() => get_library())`.
                let argument = match unthunk_callee(expr) {
                    Some(callee) => callee,
                    None => &*b.arena.alloc(b.arrow_expr(expr)),
                };
                Some(b.member_call("$", "derived", std::slice::from_ref(argument)))
            }
            Some(RuneInit::DerivedBy(f)) => {
                walk_expression_guarded(f, &mut ctx)?;
                Some(b.member_call("$", "derived", std::slice::from_ref(f)))
            }
            None => {
                if let Some(init) = &declarator.init {
                    walk_expression_guarded(init, &mut ctx)?;
                }
                declarator.init.clone()
            }
        };
        declarations.push(VariableDeclarator {
            id: new_id,
            init: new_init,
            definite: declarator.definite,
            span: declarator.span,
        });
    }
    Ok(Some(Statement::VariableDeclaration(VariableDeclaration {
        kind: decl.kind,
        declarations: declarations.into_bump_slice(),
        declare: decl.declare,
        span: decl.span,
    })))
}

/// The oracle injects `$$slots, $$events` into the `$props()` binding pattern
/// wherever a rest element captures the remaining props (probe-verified):
///
/// - `let {a, ...rest} = $props()` →
///   `let { a, $$slots, $$events, ...rest } = $$props;` — injected immediately
///   BEFORE the rest element;
/// - `let props = $props()` (non-destructured) →
///   `let { $$slots, $$events, ...props } = $$props;`;
/// - a plain destructure without a rest element gets NO injection.
///
/// Returns the replacement pattern, or `None` to keep the original borrowed
/// one. Refuses a non-identifier/non-object `$props()` pattern (the oracle
/// rejects those — props_invalid_identifier) and injection alongside carried
/// comments (the minted properties' appendix spans between host-span siblings
/// would sweep host comments).
///
/// When the component references `$$slots` (`uses_slots`), the injected
/// sanitize_slots const owns that name, so the destructured prop deconflicts by
/// renaming: `$$slots: $$slots_` (the oracle's `VariableDeclaration.js:56-73`
/// rule — always the `_` suffix, unconditional; `$$events` never renames, and a
/// user `$$slots_`/`$$events` reference or declaration is oracle-rejected input,
/// so no second-order collision exists).
fn inject_props_pattern<'arena>(
    b: &mut Builder<'arena>,
    id: &'arena Expression<'arena>,
    has_comments: bool,
    uses_slots: bool,
) -> Result<Option<Expression<'arena>>, CompileError> {
    match id {
        Expression::ObjectPattern(obj) => {
            let has_rest = obj
                .properties
                .iter()
                .any(|p| matches!(p, ObjectPatternProperty::RestElement(_)));
            if !has_rest {
                return Ok(None);
            }
            if has_comments {
                return Err(unsupported(Refusal::CommentsWithRestProps));
            }
            let mut properties: BumpVec<'arena, ObjectPatternProperty<'arena>> =
                BumpVec::new_in(b.arena);
            for prop in obj.properties {
                if matches!(prop, ObjectPatternProperty::RestElement(_)) {
                    properties.push(slots_pattern_prop(b, uses_slots));
                    properties.push(shorthand_pattern_prop(b, "$$events"));
                }
                properties.push(prop.clone());
            }
            Ok(Some(Expression::ObjectPattern(ObjectPattern {
                properties: properties.into_bump_slice(),
                optional: obj.optional,
                type_annotation: obj.type_annotation.clone(),
                decorators: obj.decorators,
                span: obj.span,
            })))
        }
        Expression::Identifier(_) => {
            if has_comments {
                return Err(unsupported(Refusal::CommentsWithNonDestructuredProps));
            }
            let mut properties: BumpVec<'arena, ObjectPatternProperty<'arena>> =
                BumpVec::new_in(b.arena);
            properties.push(slots_pattern_prop(b, uses_slots));
            properties.push(shorthand_pattern_prop(b, "$$events"));
            properties.push(ObjectPatternProperty::RestElement(RestElement {
                argument: b.arena.alloc(id.clone()),
                optional: false,
                type_annotation: None,
                span: id.span(),
            }));
            Ok(Some(Expression::ObjectPattern(ObjectPattern {
                properties: properties.into_bump_slice(),
                optional: false,
                type_annotation: None,
                decorators: None,
                span: id.span(),
            })))
        }
        _ => Err(unsupported(Refusal::PropsBindingPattern)),
    }
}

/// The injected `$$slots` pattern property: shorthand `{ $$slots }` normally,
/// renamed `{ $$slots: $$slots_ }` when the sanitize_slots const owns the name
/// (see `inject_props_pattern`).
fn slots_pattern_prop<'arena>(
    b: &mut Builder<'arena>,
    uses_slots: bool,
) -> ObjectPatternProperty<'arena> {
    if !uses_slots {
        return shorthand_pattern_prop(b, "$$slots");
    }
    let key = b.ident("$$slots");
    b.mint(": ");
    let value = b.ident("$$slots_");
    let span = Span::new(key.span.start, value.span.end);
    ObjectPatternProperty::Property(Property {
        key: Expression::Identifier(key),
        value: Expression::Identifier(value),
        kind: PropertyKind::Init,
        shorthand: false,
        computed: false,
        method: false,
        span,
    })
}

/// A shorthand `{ name }` pattern property over a synthetic identifier
/// (interned name; the span is the minted appendix text).
fn shorthand_pattern_prop<'arena>(
    b: &mut Builder<'arena>,
    name: &str,
) -> ObjectPatternProperty<'arena> {
    let ident = b.ident(name);
    let span = ident.span;
    ObjectPatternProperty::Property(Property {
        key: Expression::Identifier(ident.clone()),
        value: Expression::Identifier(ident),
        kind: PropertyKind::Init,
        shorthand: true,
        computed: false,
        method: false,
        span,
    })
}

/// A statement body under construction: the statements emitted so far plus the
/// pending template accumulator (alternating static text and interpolation
/// expressions, `texts.len() == exprs.len() + 1` — the
/// [`Builder::template_literal`] shape). Control-flow blocks `flush` the
/// pending template into a `$$renderer.push(…)` statement, emit their own
/// statements, and let closer-anchor text accumulate into the next template —
/// the oracle's multi-push output shape.
struct BodyBuilder<'arena> {
    stmts: BumpVec<'arena, Statement<'arena>>,
    texts: Vec<String>,
    exprs: BumpVec<'arena, Expression<'arena>>,
}

impl<'arena> BodyBuilder<'arena> {
    fn new_in(arena: &'arena bumpalo::Bump) -> Self {
        Self {
            stmts: BumpVec::new_in(arena),
            texts: vec![String::new()],
            exprs: BumpVec::new_in(arena),
        }
    }

    /// Append an already template-escaped chunk to the current static part.
    ///
    /// **The cross-chunk `${` seam.** Each chunk is template-escaped on its own
    /// (`escape_template_text` rewrites `$` to `\$` only when it sees the `{`
    /// itself), so a literal `$` *ending* one chunk and a literal `{` *starting*
    /// the next slip through as a live interpolation — the emitted
    /// `` $$renderer.push(`… ${NAME} …`) `` would then evaluate `NAME`, or fail to
    /// parse. Real: `ssh ${'{'}DEPLOY_USER}` writes a shell variable by folding a
    /// `'{'` string literal into the text right after a `$`. The oracle escapes it
    /// (it assembles the whole string before escaping); tsv joins the seam here.
    fn push_text(&mut self, chunk: &str) {
        // Every element of `texts` exists by construction (starts with one entry;
        // `push_expr` appends the follower).
        #[allow(clippy::unwrap_used)]
        let current = self.texts.last_mut().unwrap();
        if current.ends_with('$') && chunk.starts_with('{') {
            // The trailing `$` is raw (any preceding backslash was already
            // doubled), so escaping it here is the identity escape `\$` — the
            // rendered text is unchanged, the interpolation is not.
            current.pop();
            current.push_str("\\$");
        }
        current.push_str(chunk);
    }

    fn push_expr(&mut self, expr: Expression<'arena>) {
        self.exprs.push(expr);
        self.texts.push(String::new());
    }

    /// Flush the pending template (if any) into a `$$renderer.push(…)`
    /// statement.
    fn flush(&mut self, b: &mut Builder<'arena>, arena: &'arena bumpalo::Bump) {
        if self.exprs.is_empty() && self.texts.iter().all(String::is_empty) {
            return;
        }
        let texts = std::mem::replace(&mut self.texts, vec![String::new()]);
        let exprs = std::mem::replace(&mut self.exprs, BumpVec::new_in(arena));
        let template = b.template_literal(&texts, exprs.into_bump_slice());
        let template_alloc = arena.alloc(template);
        let push_call = b.member_call("$$renderer", "push", std::slice::from_ref(template_alloc));
        let span = push_call.span();
        self.stmts
            .push(Statement::ExpressionStatement(ExpressionStatement {
                expression: push_call,
                span,
                is_directive: false,
            }));
    }

    /// Flush the pending template, then append a statement.
    fn push_statement(
        &mut self,
        b: &mut Builder<'arena>,
        arena: &'arena bumpalo::Bump,
        stmt: Statement<'arena>,
    ) {
        self.flush(b, arena);
        self.stmts.push(stmt);
    }

    /// Finish: flush and return the statement slice.
    fn finish(
        mut self,
        b: &mut Builder<'arena>,
        arena: &'arena bumpalo::Bump,
    ) -> &'arena [Statement<'arena>] {
        self.flush(b, arena);
        self.stmts.into_bump_slice()
    }
}

/// Svelte's template whitespace class (`[ \t\r\n]` — the compiler's
/// `regex_*_whitespaces` patterns; deliberately narrower than Unicode
/// whitespace, so e.g. a decoded `&nbsp;` is content).
fn is_template_ws(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\r' | '\n')
}

fn is_ws_only(s: &str) -> bool {
    s.chars().all(is_template_ws)
}

/// Replace the leading `[ \t\r\n]+` run with `replacement` (no-op without one).
fn replace_leading_ws(s: &str, replacement: &str) -> String {
    let trimmed = s.trim_start_matches(is_template_ws);
    if trimmed.len() == s.len() {
        s.to_string()
    } else {
        format!("{replacement}{trimmed}")
    }
}

/// Replace the trailing `[ \t\r\n]+` run with `replacement` (no-op without one).
fn replace_trailing_ws(s: &str, replacement: &str) -> String {
    let trimmed = s.trim_end_matches(is_template_ws);
    if trimmed.len() == s.len() {
        s.to_string()
    } else {
        format!("{trimmed}{replacement}")
    }
}

/// HTML-escape text content the way the oracle does (`escape_html`, `[&<]`).
fn escape_html_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}

/// HTML-escape a static attribute value (`escape_html(value, true)`, `[&"<]`).
fn escape_html_attr(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '"' => out.push_str("&quot;"),
            '<' => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}

/// A fragment child after comment-dropping, const-tag hoisting, and text
/// decoding, mutable for the whitespace normalization pass. Blocks are non-text
/// nodes for whitespace purposes (`is_expr` is false).
enum CleanNode<'arena> {
    Text(String),
    Expr(&'arena ExpressionTag<'arena>),
    Html(&'arena HtmlTag<'arena>),
    Element(&'arena Element<'arena>),
    If(&'arena IfBlock<'arena>),
    Each(&'arena EachBlock<'arena>),
    Await(&'arena AwaitBlock<'arena>),
    Key(&'arena KeyBlock<'arena>),
    Render(&'arena RenderTag<'arena>),
}

impl CleanNode<'_> {
    fn is_expr(&self) -> bool {
        // Only `{expr}` tags count as text for whitespace purposes — the
        // oracle's `prev?.type !== 'ExpressionTag'` checks; `{@html}` is a
        // regular non-text node there.
        matches!(self, CleanNode::Expr(_))
    }
}

/// Per-fragment emission context.
#[allow(clippy::struct_excessive_bools)] // independent per-fragment flags, not a state machine
struct FragmentCtx<'p> {
    /// Whether a text-first fragment gets the leading `<!---->` anchor. True for
    /// the component root and `{#each}` bodies (the oracle's `is_text_first`:
    /// parent ∈ {Fragment, SnippetBlock, EachBlock, Component, …}); false for
    /// element children and `{#if}`/`{#key}`/`{#await}` bodies.
    mark_text_first: bool,
    /// Whether this is the component's root fragment (a `{@const}` here refuses —
    /// grammatically block-only, and its component-scope placement is unprobed).
    is_component_root: bool,
    /// Whether this fragment is a **block scope** (component root, a block body,
    /// or a `<svelte:head>` closure) that owns snippet hoisting. The oracle
    /// hoists a `{#snippet}` to its nearest enclosing block scope, bubbling
    /// *through* elements (which share the block's `init`), so a block-scope
    /// fragment collects snippets from its whole element subtree and emits their
    /// `function` declarations at the front; an element-child fragment
    /// (`hoist_snippets = false`) leaves its snippets to the enclosing block.
    hoist_snippets: bool,
    /// The enclosing scope's `is_standalone` (the oracle's `clean_nodes`
    /// `is_standalone`, inherited by element children). A block scope recomputes
    /// it from its own trimmed list; an element child inherits it. When true, a
    /// sole `{@render}` reuses the parent block's anchor and emits no trailing
    /// `<!---->`. An element wrapping the render makes the enclosing block's sole
    /// child the element (not a render), so the inherited value is false.
    is_standalone: bool,
    /// Inside `<pre>`/`<textarea>`: no whitespace normalization.
    preserve_whitespace: bool,
    /// The enclosing element's name (`None` at the root).
    parent_name: Option<&'p str>,
}

/// Walk a fragment: normalize whitespace per the oracle's `clean_nodes` rules,
/// then append static HTML / interpolations to the template.
fn emit_fragment<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    fragment: &Fragment<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let nodes: &'arena [FragmentNode<'arena>] = fragment.nodes;
    let source = env.source;

    // Decode and filter into the working list. Comments are dropped (the oracle
    // compiles with preserveComments off); `{@const}` tags are hoisted out of the
    // whitespace list and emitted first (the oracle's `clean_nodes` hoisting).
    let mut list: Vec<CleanNode<'arena>> = Vec::with_capacity(nodes.len());
    let mut const_tags: Vec<&'arena ConstTag<'arena>> = Vec::new();
    let mut head_nodes: Vec<&'arena SpecialElement<'arena>> = Vec::new();
    for node in nodes {
        match node {
            FragmentNode::SpecialElement(se)
                if matches!(se.kind, SpecialElementKind::SvelteHead) =>
            {
                head_nodes.push(se);
            }
            FragmentNode::Text(text) => {
                list.push(CleanNode::Text(text.data(source).into_owned()));
            }
            FragmentNode::Element(element) => list.push(CleanNode::Element(element)),
            FragmentNode::ExpressionTag(tag) => list.push(CleanNode::Expr(tag)),
            FragmentNode::HtmlTag(tag) => list.push(CleanNode::Html(tag)),
            FragmentNode::IfBlock(block) => list.push(CleanNode::If(block)),
            FragmentNode::EachBlock(block) => list.push(CleanNode::Each(block)),
            FragmentNode::AwaitBlock(block) => list.push(CleanNode::Await(block)),
            FragmentNode::KeyBlock(block) => list.push(CleanNode::Key(block)),
            FragmentNode::RenderTag(tag) => list.push(CleanNode::Render(tag)),
            // Snippets are hoisted to the enclosing block scope (below), not
            // emitted inline — skip them in the template list.
            FragmentNode::SnippetBlock(_) => {}
            FragmentNode::ConstTag(tag) => {
                if ctx.is_component_root {
                    return Err(unsupported(Refusal::ConstTagAtRoot));
                }
                const_tags.push(tag);
            }
            FragmentNode::Comment(_) => {}
            other => {
                return Err(unsupported(Refusal::TemplateNode {
                    kind: fragment_node_kind(other),
                }));
            }
        }
    }

    // Snippets hoist to their nearest enclosing block scope, bubbling through
    // elements (which share the block's `init`): a block-scope fragment collects
    // its whole element subtree's snippets and emits their `function`
    // declarations first (hoistable top-level ones to module scope, the rest to
    // this block's init); an element-child fragment leaves them to the block.
    let hoisted_snippets = if ctx.hoist_snippets {
        let mut collected: Vec<&'arena SnippetBlock<'arena>> = Vec::new();
        collect_hoisted_snippets(fragment, &mut collected);
        collected
    } else {
        Vec::new()
    };

    // Emit hoisted `{@const}` declarations first — they precede the anchor's
    // following content and enter the evaluator's innermost overlay so later
    // reads in this fragment fold. `<svelte:head>` is hoisted the same way; a
    // fragment carrying both can't fix their relative order (the oracle keeps
    // source order across all hoisted kinds), so refuse that combination.
    if !head_nodes.is_empty() && !const_tags.is_empty() {
        return Err(unsupported(Refusal::SvelteHeadWithConstTag));
    }
    // A snippet sharing a block with a `{@const}`/`<svelte:head>` can't fix the
    // relative hoist order across kinds — refuse the mix.
    if !hoisted_snippets.is_empty() && (!const_tags.is_empty() || !head_nodes.is_empty()) {
        return Err(unsupported(Refusal::SnippetHoistOrder));
    }
    for tag in &const_tags {
        emit_const_tag(env, tag, out)?;
    }
    for head in &head_nodes {
        emit_svelte_head(env, head, out)?;
    }
    for snippet in &hoisted_snippets {
        emit_snippet(env, snippet, out)?;
    }

    if !ctx.preserve_whitespace {
        normalize_whitespace(&mut list, ctx.parent_name);
    }

    // A lone leading newline text in <pre> is dropped (the browser would drop
    // it too, which would otherwise break hydration).
    if ctx.parent_name == Some("pre")
        && let Some(CleanNode::Text(data)) = list.first()
        && (data == "\n" || data == "\r\n")
    {
        list.remove(0);
    }

    // A text-first fragment gets a leading `<!---->` so its text doesn't glue to
    // the surrounding SSR fragment (component root and `{#each}` bodies only).
    if ctx.mark_text_first && matches!(list.first(), Some(CleanNode::Text(_) | CleanNode::Expr(_)))
    {
        out.push_text("<!---->");
    }

    // `is_standalone` (the oracle's `clean_nodes` flag) is recomputed at a block
    // scope from its own trimmed list — a sole non-dynamic `{@render}` reuses the
    // parent block's anchor — and *inherited* by element children (an element
    // wrapping the render already made the block's sole child the element, so the
    // inherited value is false).
    let is_standalone = if ctx.hoist_snippets {
        match list.as_slice() {
            // `render_callee_name` is a SHAPE predicate (it wants a plain-identifier
            // callee), so it must read the ERASED expression — the oracle strips
            // TypeScript before `clean_nodes` runs, and to it `{@render (s as T)(x)}`
            // is a plain `s(x)`. Reading the raw node would call it dynamic and emit
            // an anchor the oracle elides. `emit_render_tag` erases again for its own
            // borrow; a second erase of a type-free node allocates nothing.
            [CleanNode::Render(tag)] => {
                let expression = env.erase(&tag.expression)?;
                render_callee_name(expression, source)
                    .is_some_and(|name| !render_callee_dynamic(env, name))
            }
            [CleanNode::Element(element)] => component_is_standalone_eligible(env, element),
            _ => false,
        }
    } else {
        ctx.is_standalone
    };

    for node in &list {
        match node {
            CleanNode::Text(data) => {
                out.push_text(&escape_template_text(&escape_html_text(data)));
            }
            CleanNode::Element(element) => {
                emit_element(env, element, out, &ctx, is_standalone)?;
            }
            CleanNode::Expr(tag) => {
                emit_expression_tag(env, &tag.expression, out, true)?;
            }
            CleanNode::Html(tag) => {
                emit_expression_tag(env, &tag.expression, out, false)?;
            }
            CleanNode::If(block) => emit_if_block(env, block, out, &ctx)?,
            CleanNode::Each(block) => emit_each_block(env, block, out, &ctx)?,
            CleanNode::Await(block) => emit_await_block(env, block, out, &ctx)?,
            CleanNode::Key(block) => emit_key_block(env, block, out, &ctx)?,
            CleanNode::Render(tag) => emit_render_tag(env, tag, out, is_standalone)?,
        }
    }
    Ok(())
}

/// Recursively test whether a fragment contains any control-flow block or
/// `{@const}` tag (the comments+blocks refusal gate).
fn fragment_contains_block(fragment: &Fragment<'_>) -> bool {
    fragment.nodes.iter().any(|node| match node {
        FragmentNode::IfBlock(_)
        | FragmentNode::EachBlock(_)
        | FragmentNode::AwaitBlock(_)
        | FragmentNode::KeyBlock(_)
        | FragmentNode::ConstTag(_) => true,
        FragmentNode::Element(element) => fragment_contains_block(&element.fragment),
        _ => false,
    })
}

/// Recursively test whether a fragment contains a `{#snippet}` or `{@render}`
/// (the comments+snippet/render refusal gate — a hoisted snippet function or a
/// per-render flush reshapes the body in ways whose comment windows aren't
/// probed).
fn fragment_has_snippet_or_render(fragment: &Fragment<'_>) -> bool {
    fragment.nodes.iter().any(|node| match node {
        FragmentNode::SnippetBlock(_) | FragmentNode::RenderTag(_) => true,
        FragmentNode::Element(element) => fragment_has_snippet_or_render(&element.fragment),
        FragmentNode::SpecialElement(se) => fragment_has_snippet_or_render(&se.fragment),
        FragmentNode::IfBlock(b) => {
            fragment_has_snippet_or_render(&b.consequent)
                || b.alternate
                    .as_ref()
                    .is_some_and(fragment_has_snippet_or_render)
        }
        FragmentNode::EachBlock(b) => {
            fragment_has_snippet_or_render(&b.body)
                || b.fallback
                    .as_ref()
                    .is_some_and(fragment_has_snippet_or_render)
        }
        FragmentNode::AwaitBlock(b) => [&b.pending, &b.then, &b.catch]
            .into_iter()
            .flatten()
            .any(fragment_has_snippet_or_render),
        FragmentNode::KeyBlock(b) => fragment_has_snippet_or_render(&b.fragment),
        _ => false,
    })
}

/// Emit a block body fragment into a fresh child body builder, prepending `pre`
/// statements (block anchor pushes, an `{#each}` binding) and pushing a
/// block-scope `overlay` (empty for `{#if}`/`{#key}`, seeded with masked locals
/// for `{#each}`/`{#await}`), and return the finished statement slice. The
/// overlay gives any `{@const}` in the body a scope to enter.
fn emit_child_body<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    fragment: &Fragment<'arena>,
    pre: &[Statement<'arena>],
    mark_text_first: bool,
    preserve_whitespace: bool,
    overlay: HashMap<String, ScopeEntry<'arena>>,
) -> Result<&'arena [Statement<'arena>], CompileError> {
    let arena = env.b.arena;
    env.push_overlay(overlay)?;
    let mut child = BodyBuilder::new_in(arena);
    for stmt in pre {
        child.stmts.push(stmt.clone());
    }
    let result = emit_fragment(
        env,
        fragment,
        &mut child,
        FragmentCtx {
            mark_text_first,
            is_component_root: false,
            hoist_snippets: true,
            is_standalone: false,
            preserve_whitespace,
            parent_name: None,
        },
    );
    env.pop_overlay();
    result?;
    Ok(child.finish(&mut env.b, arena))
}

/// Prepare a single borrowed value expression for a read position (`{#if}` test,
/// `{#each}` collection, `{#await}` promise): a bare derived read becomes `d()`,
/// everything else is guarded and passed through borrowed.
fn wrap_single<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<Expression<'arena>, CompileError> {
    let wrapped = wrap_value_expr(env, expr)?;
    Ok(wrapped[0].clone())
}

/// The guard for an expression the SSR output **drops** — the `{#each}` key, the
/// `{#key}` expression, an event-handler attribute, and everything inside a
/// `{:catch}` branch.
///
/// A misplaced rune must still refuse: the oracle rejects one in its *analysis*
/// phase, before it decides what to emit, so dropping the region cannot make the
/// component valid (`{:catch e}{$state(1)}{/await}` is a `state_invalid_placement`
/// error there).
///
/// But the derived-read rule is switched **off** — the empty `derived_names` set.
/// It is an emission *rewrite* (a bare read becomes `d()`; `wrap_value_expr`
/// applies it before the guard runs, so `walk_expression_guarded` refuses every
/// derived read that reaches it), not a validity rule: the oracle happily accepts
/// a derived read it never emits. Enforcing it in a dropped region would refuse
/// `{#key d}` and `{:catch e}<p>{d}</p>`, which the oracle compiles.
fn guard_dropped<'arena>(
    env: &EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    // The component-wide reassignment/shadow collection is `needs_context`'s job
    // (it walks the dropped regions too), so these two are throwaways.
    let no_derived = NameSet::default();
    let mut ctx = WalkCtx::new(env.source, &mut updated, &mut nested, &no_derived);
    walk_expression_guarded(expr, &mut ctx)
}

/// The guard for a binding **pattern** the SSR output EMITS verbatim — the
/// `{#each}` context (`let CTX = each_array[i]`) and the `{:then}` value (the
/// then-arrow's parameter).
///
/// A pattern is not a dropped region: its *default values* are real emitted
/// expressions (`{#each xs as { a = d }}`), and this emitter borrows the pattern
/// through untouched — it never rewrites a derived read inside one to `d()` the
/// way `wrap_value_expr` does for a value position. So the derived rule stays ON
/// here (unlike [`guard_dropped`]): a derived read in a pattern default would
/// otherwise emit a bare `d` where the oracle emits `d()`. A MISMATCH, so refuse.
fn guard_pattern<'arena>(
    env: &EmitEnv<'arena, '_>,
    pattern: &'arena Expression<'arena>,
) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let mut ctx = WalkCtx::new(env.source, &mut updated, &mut nested, &env.derived_names);
    walk_expression_guarded(pattern, &mut ctx)
}

/// Guard a whole fragment the SSR output drops (the `{:catch}` branch) — the
/// dropped-fragment analog of [`guard_dropped`], over `attr_refs`' shared
/// traversal. Without it a rune anywhere inside a `{:catch}` compiles, which the
/// oracle rejects: the M4 lesson (an emission-dropped fragment still needs
/// refusal-equivalent walking), and the property `dropped_fragments_are_walked`
/// pins.
fn guard_dropped_fragment<'arena>(
    env: &EmitEnv<'arena, '_>,
    fragment: &'arena Fragment<'arena>,
) -> Result<(), CompileError> {
    each_template_item(fragment, &mut |item| match item {
        TemplateItem::Expression(expr) => guard_dropped(env, expr),
        // A `<T>` clause holds no value reference. TypeScript in a document with
        // no `lang="ts"` is refused up front by `refuse_template_typescript`.
        TemplateItem::SnippetTypeParameters => Ok(()),
    })
}

/// Refuse if a generated block name (`each_array`, `$$index`, `$$length`) would
/// collide with a user binding — the oracle's component-scope name generation
/// would then pick a different suffix, which this port doesn't replicate.
fn check_name_free(env: &EmitEnv<'_, '_>, name: &str) -> Result<(), CompileError> {
    if env.bindings.contains(name) || env.snippets.names.contains(name) {
        return Err(unsupported(Refusal::GeneratedNameCollision {
            name: name.to_string(),
        }));
    }
    Ok(())
}

/// A single-declarator `let`/`const` declaration statement.
/// The filename the deterministic oracle compiles under. `$.head`'s dedup hash
/// is `hash(filename)` (`SvelteHead.js`), so a fixed filename makes it constant.
const COMPILE_FILENAME: &str = "input.svelte";

/// Port of Svelte's `hash` (`utils.js`): strip carriage returns, then a djb2-xor
/// fold over the code units in reverse, rendered base-36. Used for `$.head`.
///
/// Folds over `chars()` (code points) where the oracle uses `charCodeAt` (UTF-16
/// code units) — identical for BMP input, divergent for astral characters. Safe
/// at the only call site (the ASCII [`COMPILE_FILENAME`] constant); revisit if a
/// real filename ever feeds this.
fn svelte_hash(s: &str) -> String {
    let mut hash: u32 = 5381;
    for ch in s.chars().rev() {
        if ch == '\r' {
            continue;
        }
        hash = hash.wrapping_shl(5).wrapping_sub(hash) ^ (ch as u32);
    }
    if hash == 0 {
        return "0".to_string();
    }
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = String::new();
    while hash > 0 {
        out.push(DIGITS[(hash % 36) as usize] as char);
        hash /= 36;
    }
    out.chars().rev().collect()
}

/// Emit `$.head(hash, $$renderer, ($$renderer) => { … })` for `<svelte:head>`.
///
/// The head fragment is a normal fragment emitted into the closure body, so a
/// `<title>` (a `TitleElement` needing `$$renderer.title`) or any other special
/// child refuses through the usual `emit_fragment` path. Attributes on the head
/// element are refused (the oracle carries none in this subset).
fn emit_svelte_head<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    head: &'arena SpecialElement<'arena>,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    if !head.attributes.is_empty() {
        return Err(unsupported(Refusal::SvelteHeadAttributes));
    }
    // The head body: a normal fragment (not text-first-marked) in the closure.
    let body = emit_child_body(env, &head.fragment, &[], false, false, HashMap::new())?;
    let here = env.b.here();
    let renderer_param = Expression::Identifier(env.b.ident("$$renderer"));
    let params = std::slice::from_ref(arena.alloc(renderer_param));
    let arrow = env.b.arrow_block(params, body, here);

    // `$.head('<hash>', $$renderer, arrow)`.
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(env.b.string_literal_expr(&svelte_hash(COMPILE_FILENAME)));
    args.push(Expression::Identifier(env.b.ident("$$renderer")));
    args.push(arrow);
    let call = env.b.member_call("$", "head", args.into_bump_slice());
    let span = call.span();
    let stmt = Statement::ExpressionStatement(ExpressionStatement {
        expression: call,
        span,
        is_directive: false,
    });
    out.push_statement(&mut env.b, arena, stmt);
    Ok(())
}

/// `const $$slots = $.sanitize_slots($$props);` — the oracle's `uses_slots`
/// binding, prepended to the component function body.
fn build_sanitize_slots_decl<'arena>(
    b: &mut Builder<'arena>,
    arena: &'arena bumpalo::Bump,
) -> Statement<'arena> {
    // Mint the id before the init so the declaration span runs forward
    // (`id.start < init.end`), the same invariant the each-array decl relies on.
    let slots_id = Expression::Identifier(b.ident("$$slots"));
    let props_ident = b.ident("$$props");
    let props_arg = arena.alloc(Expression::Identifier(props_ident));
    let init = b.member_call("$", "sanitize_slots", std::slice::from_ref(props_arg));
    declaration_stmt(b, VariableDeclarationKind::Const, slots_id, init)
}

fn declaration_stmt<'arena>(
    b: &Builder<'arena>,
    kind: VariableDeclarationKind,
    id: Expression<'arena>,
    init: Expression<'arena>,
) -> Statement<'arena> {
    let span = Span::new(id.span().start, init.span().end);
    let declarator = VariableDeclarator {
        id,
        init: Some(init),
        definite: false,
        span,
    };
    let decls = std::slice::from_ref(b.arena.alloc(declarator));
    Statement::VariableDeclaration(VariableDeclaration {
        kind,
        declarations: decls,
        declare: false,
        span,
    })
}

/// Wrap a finished statement slice in a `Statement::BlockStatement` (`{ … }`).
fn block_stmt<'arena>(
    b: &Builder<'arena>,
    body: &'arena [Statement<'arena>],
) -> &'arena Statement<'arena> {
    let span = b.here();
    b.arena
        .alloc(Statement::BlockStatement(BlockStatement { body, span }))
}

/// Emit `{@const name = init}` into the current fragment: a hoisted `const`
/// declaration plus an evaluator overlay entry so later reads fold.
fn emit_const_tag<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    tag: &'arena ConstTag<'arena>,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // Two template borrow points: the binding pattern (`{@const a: T = v}`) and
    // the initializer. The erased init feeds BOTH the emitted declaration and the
    // evaluator overlay below, so a later `{a}` read folds through the same node
    // the oracle folds.
    let id = env.erase(&tag.id)?;
    let init = env.erase(&tag.init)?;
    // Only a plain-identifier binding is modeled: a destructured `{@const}`
    // whose init folds would have the oracle fold each read, which this port
    // can't reproduce per-binding — refuse rather than risk a silent mismatch.
    let Expression::Identifier(ident) = id else {
        return Err(unsupported(Refusal::DestructuredConstTag));
    };
    let Some(name) = plain_identifier_name(ident, env.source) else {
        return Err(unsupported(Refusal::ConstTagNonPlainName));
    };
    // A `{@const}` that shadows a top-level `$derived` binding is refused, the
    // same as an each/await binding (see `push_overlay`): the derived-read
    // rewrite is name-based and would wrongly turn a later `{name}` read into a
    // `name()` call. `emit_const_tag` inserts into the overlay directly, so it
    // must repeat that check here.
    if env.derived_names.contains(&name) {
        return Err(unsupported(Refusal::BlockScopeShadowsDerived { name }));
    }
    // Guard + wrap the init (bare derived → d(), refuse runes/mutations).
    let wrapped_init = wrap_single(env, init)?;
    let id_expr = id.clone();
    let arena = env.b.arena;
    let stmt = declaration_stmt(
        &env.b,
        VariableDeclarationKind::Const,
        id_expr,
        wrapped_init,
    );
    out.push_statement(&mut env.b, arena, stmt);

    // Enter the innermost overlay so `{name}` reads fold through its init.
    let binding = Binding {
        kind: BindingKind::Normal,
        initial: Initial::Expr(init),
        updated: false,
    };
    match env.overlays.last_mut() {
        Some(overlay) => {
            overlay.insert(name, ScopeEntry::Const(binding));
        }
        None => {
            // Unreachable: root-level `{@const}` already refused in emit_fragment.
            return Err(unsupported(Refusal::ConstTagOutsideBlock));
        }
    }
    Ok(())
}

/// Emit `{#if}` / `{:else if}` / `{:else}`: a flat `if … else if … else` chain
/// with per-branch hydration anchors (`<!--[N-->`, terminal `<!--[-1-->`) and a
/// merge-forward `<!--]-->` closer. A missing `{:else}` synthesizes the
/// anchor-only terminal branch.
fn emit_if_block<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    if_block: &'arena IfBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    let preserve = ctx.preserve_whitespace;

    // Flatten the else-if chain into (test, consequent) branches + terminal else
    // (an `{:else if}` nests as an alternate fragment of one `IfBlock{elseif}`).
    let mut branches: Vec<(&'arena Expression<'arena>, &'arena Fragment<'arena>)> = Vec::new();
    let mut current = if_block;
    let final_else: Option<&'arena Fragment<'arena>>;
    loop {
        branches.push((&current.test, &current.consequent));
        match &current.alternate {
            Some(alt) => {
                if let [FragmentNode::IfBlock(inner)] = alt.nodes
                    && inner.elseif
                {
                    current = inner;
                    continue;
                }
                final_else = Some(alt);
                break;
            }
            None => {
                final_else = None;
                break;
            }
        }
    }

    // Build branch bodies in document order (anchors 0,1,2,…) so nested-block
    // name counters advance in the oracle's order.
    let mut cons_blocks: Vec<(Expression<'arena>, &'arena Statement<'arena>)> = Vec::new();
    for (i, &(test, frag)) in branches.iter().enumerate() {
        let test = env.erase(test)?;
        let test_expr = wrap_single(env, test)?;
        let anchor = env.b.push_string_stmt(&format!("<!--[{i}-->"));
        let body = emit_child_body(
            env,
            frag,
            std::slice::from_ref(&anchor),
            false,
            preserve,
            HashMap::new(),
        )?;
        let block = block_stmt(&env.b, body);
        cons_blocks.push((test_expr, block));
    }

    // Terminal else (document order: after every consequent).
    let else_anchor = env.b.push_string_stmt("<!--[-1-->");
    let else_body = match final_else {
        Some(frag) => emit_child_body(
            env,
            frag,
            std::slice::from_ref(&else_anchor),
            false,
            preserve,
            HashMap::new(),
        )?,
        None => {
            let mut v: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
            v.push(else_anchor);
            v.into_bump_slice()
        }
    };
    let mut alternate: &'arena Statement<'arena> = block_stmt(&env.b, else_body);

    // Assemble the chain inner-to-outer.
    for (test, cons) in cons_blocks.into_iter().rev() {
        let here = env.b.here();
        let if_stmt = IfStatement {
            test,
            consequent: cons,
            alternate: Some(alternate),
            span: here,
        };
        alternate = arena.alloc(Statement::IfStatement(if_stmt));
    }

    out.push_statement(&mut env.b, arena, (*alternate).clone());
    out.push_text("<!--]-->");
    Ok(())
}

/// Emit `{#each}`: `const each_array = $.ensure_array_like(expr)` + a `for` loop
/// binding `let CTX = each_array[IDX]`. Without `{:else}` the opener `<!--[-->`
/// merges into the preceding template; with it, `each_array` hoists before an
/// `if (each_array.length !== 0) { … } else { … }` whose openers are string
/// pushes. Nested `{#each}` refuses (unique-name order not reproducible).
fn emit_each_block<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    each: &'arena EachBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    if env.in_each {
        return Err(unsupported(Refusal::NestedEach));
    }
    let arena = env.b.arena;
    let preserve = ctx.preserve_whitespace;

    // The key `(key)` and the context pattern are guard-walked (a rune could hide
    // in either); the key is then dropped — SSR ignores it, so its TypeScript
    // (`{#each xs as x (k as string)}`) never reaches output and needs no erasure.
    // The guard walk carries its own TypeScript-unwrap arms for exactly this.
    if let Some(key) = &each.key {
        guard_dropped(env, key)?;
    }
    // The context pattern IS borrowed into the emitted `let CTX = each_array[IDX]`
    // — a template borrow point (`{#each xs as x: T}`).
    let context = match &each.context {
        Some(context) => {
            let context = env.erase(context)?;
            guard_pattern(env, context)?;
            Some(context)
        }
        None => None,
    };

    // Collection (guard + bare-derived rewrite).
    let collection_expr = env.erase(&each.expression)?;
    let collection = wrap_single(env, collection_expr)?;

    // Unique names: both counters advance once per each block (lockstep with the
    // oracle's per-each `scope.generate`, so `$$index` advances even when the
    // index is authored). `$$length` is a fixed block-scoped name.
    let array_name = env.next_each_array_name();
    let generated_index = env.next_index_name();
    let index_name = match each.index {
        Some(i) => i.to_string(),
        None => generated_index,
    };
    check_name_free(env, &array_name)?;
    check_name_free(env, &index_name)?;
    check_name_free(env, "$$length")?;

    // const each_array = $.ensure_array_like(collection);
    // Mint the `each_array` id BEFORE the call so the declaration span runs
    // forward (id.start < init.end) — the printer's call-head width math
    // subtracts the two and would underflow on an inverted span.
    let array_id = Expression::Identifier(env.b.ident(&array_name));
    let coll_alloc = arena.alloc(collection);
    let ensure = env
        .b
        .member_call("$", "ensure_array_like", std::slice::from_ref(coll_alloc));
    let const_each = declaration_stmt(&env.b, VariableDeclarationKind::Const, array_id, ensure);

    // for-loop init: `let IDX = 0, $$length = each_array.length`.
    let index_id = Expression::Identifier(env.b.ident(&index_name));
    let zero = env.b.number(0.0);
    let index_span = Span::new(index_id.span().start, zero.span().end);
    let index_declarator = VariableDeclarator {
        id: index_id,
        init: Some(zero),
        definite: false,
        span: index_span,
    };
    let length_id = Expression::Identifier(env.b.ident("$$length"));
    let arr_for_length = env.b.ident_expr(&array_name);
    let length_member = env.b.member_prop(arr_for_length, "length");
    let length_span = Span::new(length_id.span().start, length_member.span().end);
    let length_declarator = VariableDeclarator {
        id: length_id,
        init: Some(length_member),
        definite: false,
        span: length_span,
    };
    let mut init_decls: BumpVec<'arena, VariableDeclarator<'arena>> = BumpVec::new_in(arena);
    init_decls.push(index_declarator);
    init_decls.push(length_declarator);
    let init_here = env.b.here();
    let init_decl = VariableDeclaration {
        kind: VariableDeclarationKind::Let,
        declarations: init_decls.into_bump_slice(),
        declare: false,
        span: init_here,
    };

    // test: `IDX < $$length`; update: `IDX++`.
    let idx_test = env.b.ident_expr(&index_name);
    let len_test = env.b.ident_expr("$$length");
    let test = env.b.binary(idx_test, BinaryOperator::LessThan, len_test);
    let idx_update = env.b.ident_expr(&index_name);
    let update = env.b.update(idx_update, UpdateOperator::Increment);

    // Body: `let CTX = each_array[IDX]` + the body fragment (text-first marker),
    // with context/index names masked to UNKNOWN in the evaluator.
    let mut overlay: HashMap<String, ScopeEntry<'arena>> = HashMap::new();
    if let Some(context) = context {
        let mut names = Vec::new();
        pattern_binding_names(context, env.source, &mut names)?;
        for name in names {
            overlay.insert(name, ScopeEntry::Masked);
        }
    }
    overlay.insert(index_name.clone(), ScopeEntry::Masked);
    env.in_each = true;
    let body_result = emit_each_body(
        env,
        each,
        context,
        &array_name,
        &index_name,
        preserve,
        overlay,
    );
    env.in_each = false;
    let body_stmts = body_result?;

    let for_body = block_stmt(&env.b, body_stmts);
    let for_here = env.b.here();
    let for_loop = Statement::ForStatement(ForStatement {
        init: Some(ForInit::VariableDeclaration(init_decl)),
        test: Some(test),
        update: Some(update),
        body: for_body,
        span: for_here,
    });

    if let Some(fallback) = &each.fallback {
        out.push_statement(&mut env.b, arena, const_each);
        // if branch: `{ $$renderer.push('<!--[-->'); for_loop }`.
        let open_anchor = env.b.push_string_stmt("<!--[-->");
        let mut if_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        if_body.push(open_anchor);
        if_body.push(for_loop);
        let if_branch = block_stmt(&env.b, if_body.into_bump_slice());
        // else branch: `{ $$renderer.push('<!--[!-->'); …fallback… }`. The
        // fallback's parent is the each block, so it is text-first-eligible.
        let else_anchor = env.b.push_string_stmt("<!--[!-->");
        let fallback_stmts = emit_child_body(
            env,
            fallback,
            std::slice::from_ref(&else_anchor),
            true,
            preserve,
            HashMap::new(),
        )?;
        let else_branch = block_stmt(&env.b, fallback_stmts);
        // condition: `each_array.length !== 0`.
        let arr_cond = env.b.ident_expr(&array_name);
        let len_cond = arena.alloc(env.b.member_prop(arr_cond, "length"));
        let zero_cond = arena.alloc(env.b.number(0.0));
        let cond = env
            .b
            .binary(len_cond, BinaryOperator::BangEqualsEquals, zero_cond);
        let if_here = env.b.here();
        let if_stmt = Statement::IfStatement(IfStatement {
            test: cond,
            consequent: if_branch,
            alternate: Some(else_branch),
            span: if_here,
        });
        out.push_statement(&mut env.b, arena, if_stmt);
    } else {
        // Opener merges into the preceding template; then const + for loop.
        out.push_text("<!--[-->");
        out.push_statement(&mut env.b, arena, const_each);
        out.push_statement(&mut env.b, arena, for_loop);
    }
    out.push_text("<!--]-->");
    Ok(())
}

/// The `{#each}` body: `let CTX = each_array[IDX]` (when `as` is present) then
/// the body fragment (which gets the text-first `<!---->` marker). `context` is
/// the **erased** binding pattern.
fn emit_each_body<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    each: &'arena EachBlock<'arena>,
    context: Option<&'arena Expression<'arena>>,
    array_name: &str,
    index_name: &str,
    preserve: bool,
    overlay: HashMap<String, ScopeEntry<'arena>>,
) -> Result<&'arena [Statement<'arena>], CompileError> {
    let arena = env.b.arena;
    let mut pre: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    if let Some(context) = context {
        let arr = env.b.ident_expr(array_name);
        let idx = env.b.ident_expr(index_name);
        let member = env.b.member_computed(arr, idx);
        let let_stmt = declaration_stmt(
            &env.b,
            VariableDeclarationKind::Let,
            context.clone(),
            member,
        );
        pre.push(let_stmt);
    }
    emit_child_body(env, &each.body, &pre, true, preserve, overlay)
}

/// Emit `{#await}`: `$.await($$renderer, expr, () => {pending}, (value?) => {then})`
/// followed by a merge-forward `<!--]-->` closer. The `{:catch}` branch is
/// dropped (the oracle omits it from SSR); empty callbacks are `() => {}`.
fn emit_await_block<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    await_block: &'arena AwaitBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    let preserve = ctx.preserve_whitespace;

    let promise = env.erase(&await_block.expression)?;
    let expr = wrap_single(env, promise)?;
    // The `{:then value}` binding is borrowed into the then-arrow's parameter list
    // — a template borrow point (`{#await p then v: T}`). The `{:catch error}`
    // binding is NOT: the oracle drops the catch branch from SSR entirely, so
    // `await_block.error` never reaches output and needs no erasure.
    let value = match &await_block.value {
        Some(value) => Some(env.erase(value)?),
        None => None,
    };

    // Pending arrow: `() => { pending }` (empty when there is no pending content).
    let empty: &'arena [Statement<'arena>] = &[];
    let pending_stmts = match &await_block.pending {
        Some(frag) => emit_child_body(env, frag, &[], false, preserve, HashMap::new())?,
        None => empty,
    };
    let pending_here = env.b.here();
    let no_params: &'arena [Expression<'arena>] = &[];
    let pending_arrow = env.b.arrow_block(no_params, pending_stmts, pending_here);

    // Then arrow: `(value?) => { then }`. The `{:then value}` pattern binds one
    // param; its names mask to UNKNOWN in the then body.
    let then_params: &'arena [Expression<'arena>] = match value {
        Some(value) => {
            guard_pattern(env, value)?;
            std::slice::from_ref(value)
        }
        None => &[],
    };
    let then_stmts = match &await_block.then {
        Some(frag) => {
            let mut overlay: HashMap<String, ScopeEntry<'arena>> = HashMap::new();
            if let Some(value) = value {
                let mut names = Vec::new();
                pattern_binding_names(value, env.source, &mut names)?;
                for name in names {
                    overlay.insert(name, ScopeEntry::Masked);
                }
            }
            emit_child_body(env, frag, &[], false, preserve, overlay)?
        }
        None => empty,
    };
    let then_here = env.b.here();
    let then_arrow = env.b.arrow_block(then_params, then_stmts, then_here);

    // The `{:catch}` branch is dropped from SSR — the emitter never visits it. It
    // still gets the rune guard, or a misplaced rune inside it would compile where
    // the oracle's analysis phase rejects.
    if let Some(catch) = &await_block.catch {
        guard_dropped_fragment(env, catch)?;
    }

    // `$.await($$renderer, expr, pending, then)`.
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(Expression::Identifier(env.b.ident("$$renderer")));
    args.push(expr);
    args.push(pending_arrow);
    args.push(then_arrow);
    let call = env.b.member_call("$", "await", args.into_bump_slice());
    let span = call.span();
    let stmt = Statement::ExpressionStatement(ExpressionStatement {
        expression: call,
        span,
        is_directive: false,
    });
    out.push_statement(&mut env.b, arena, stmt);
    out.push_text("<!--]-->");
    Ok(())
}

/// Emit `{#key}`: a `<!---->` marker, a bare `{ … }` block wrapping the body,
/// and a closing `<!---->`. The key expression is SSR-ignored (guard-walked,
/// then dropped).
fn emit_key_block<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    key: &'arena KeyBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
    ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    let preserve = ctx.preserve_whitespace;
    guard_dropped(env, &key.expression)?;
    out.push_text("<!---->");
    let body = emit_child_body(env, &key.fragment, &[], false, preserve, HashMap::new())?;
    let block = block_stmt(&env.b, body);
    out.push_statement(&mut env.b, arena, (*block).clone());
    out.push_text("<!---->");
    Ok(())
}

/// Emit a `{#snippet name(params)}…{/snippet}` as a
/// `function name($$renderer, ...params) { … }` declaration. A hoistable
/// top-level snippet goes to module scope (`env.hoisted_snippets`); everything
/// else goes to the current fragment's init (via `out`, flushing any pending
/// template first). The snippet body reuses the fragment machinery, with the
/// parameters masked to UNKNOWN so their reads never fold.
fn emit_snippet<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    snippet: &'arena SnippetBlock<'arena>,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    let (fn_decl, name) = build_snippet_function(env, snippet)?;
    // Only a hoistable *top-level* snippet is in the hoistable map; a nested or
    // body-local snippet (`is_hoisted` false) goes to this block's init.
    if env.snippets.is_hoisted(&name) {
        env.hoisted_snippets.push(fn_decl);
    } else {
        out.push_statement(&mut env.b, arena, fn_decl);
    }
    Ok(())
}

/// Build a `{#snippet}` as a `function name($$renderer, ...params) { … }`
/// declaration (with its plain name). Refuses typed/generic snippets and escaped
/// names. Shared by the template-hoist path ([`emit_snippet`]) and the component
/// snippet-prop path (where the function lives in the component's wrapping block).
fn build_snippet_function<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    snippet: &'arena SnippetBlock<'arena>,
) -> Result<(Statement<'arena>, String), CompileError> {
    let arena = env.b.arena;
    // The snippet's signature head (`<T>(params)`) is parsed by wrapping it in a
    // synthetic `function f<T>(params) {}`. When that inner parse FAILS the AST is
    // empty and the raw text is kept instead — nothing to erase or emit, so refuse.
    if snippet.raw_parameters.is_some()
        || (snippet.type_params_raw.is_some() && snippet.type_parameters.is_none())
    {
        return Err(unsupported(Refusal::SnippetSignatureUnparsed));
    }
    // A **top-level** rest parameter is a `snippet_invalid_rest_parameter` error in
    // the oracle's analysis phase (`2-analyze/visitors/SnippetBlock.js`), which
    // scans `node.parameters` itself and never descends — so a rest element NESTED
    // in a destructuring parameter (`{#snippet s({ ...rest })}`, `s(a, [b, ...t])`)
    // is legal and compiles. Match that exactly: top level only.
    if snippet
        .parameters
        .iter()
        .any(|param| matches!(param, Expression::RestElement(_)))
    {
        return Err(unsupported(Refusal::SnippetRestParameter));
    }
    let Some(name) = snippet_name(snippet, env.source) else {
        // An escaped snippet name isn't reproducible by this port.
        return Err(unsupported(Refusal::SnippetEscapedName));
    };
    let name = name.to_string();

    // A generic `{#snippet s<T>(x: T)}` declares a *type*-level parameter only —
    // the oracle emits `function s($$renderer, x)`, the `<T>` simply gone. The
    // clause needs no erasure of its own: the synthetic `function_declaration`
    // never carries type parameters, so not reading it IS the erasure. (Without
    // `lang="ts"` the whole shape is refused up front — see
    // `refuse_template_typescript`.)

    // The parameter list is a template borrow point — the patterns are borrowed
    // verbatim into the emitted function, so `(x: string)` erases to `(x)` here.
    let mut params: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    for param in snippet.parameters {
        params.push(env.erase(param)?.clone());
    }
    let params = params.into_bump_slice();

    // Mask the parameters to UNKNOWN in the body (a parameter read never folds),
    // and refuse a parameter that shadows a `$derived` binding (the overlay push
    // enforces this).
    let mut overlay: HashMap<String, ScopeEntry<'arena>> = HashMap::new();
    for param in params {
        let mut names = Vec::new();
        pattern_binding_names(param, env.source, &mut names)?;
        for name in names {
            overlay.insert(name, ScopeEntry::Masked);
        }
    }

    // Snippet bodies are in the oracle's `is_text_first` parent set, so a
    // text-first body gets the leading `<!---->` anchor.
    let body = emit_child_body(env, &snippet.body, &[], true, false, overlay)?;

    // `($$renderer, ...params)` — the synthetic renderer first, then the erased
    // parameter patterns.
    let mut all_params: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    all_params.push(Expression::Identifier(env.b.ident("$$renderer")));
    all_params.extend_from_slice(params);
    let block_span = env.b.here();
    let fn_decl = env
        .b
        .function_declaration(&name, all_params.into_bump_slice(), body, block_span);
    Ok((fn_decl, name))
}

/// Collect the snippets that hoist to this block scope: the fragment's own
/// snippets plus those inside descendant **HTML elements** (which share the
/// block's `init`). Stops at nested blocks, special elements, and **components** —
/// those are separate scopes that collect their own (a component's snippet
/// children become its snippet props, handled by `plan_component_children`).
fn collect_hoisted_snippets<'arena>(
    fragment: &Fragment<'arena>,
    out: &mut Vec<&'arena SnippetBlock<'arena>>,
) {
    for node in fragment.nodes {
        match node {
            FragmentNode::SnippetBlock(snippet) => out.push(snippet),
            FragmentNode::Element(element) if element.kind != ElementKind::Component => {
                collect_hoisted_snippets(&element.fragment, out);
            }
            _ => {}
        }
    }
}

/// Whether a `{@render}` expression is a call — the oracle's **parse-time**
/// shape rule, so it reads the raw (un-erased) node. A TypeScript wrapper AROUND
/// the call (`{@render (s(x) as T)}`, `{@render s(x)!}`) is
/// `render_tag_invalid_expression` there, even though erasure would reveal a call
/// underneath; a wrapper around the *callee* (`{@render (s as T)(x)}`) leaves a
/// call and compiles.
fn render_expression_is_call(expr: &Expression<'_>) -> bool {
    match expr {
        Expression::CallExpression(_) => true,
        Expression::ParenthesizedExpression(p) => {
            matches!(p.expression, Expression::CallExpression(_))
        }
        _ => false,
    }
}

/// The plain callee name of a `{@render}` expression: unwrap the (possibly
/// optional) call, requiring a plain-identifier callee. `None` for a member
/// callee, a non-call, or an escaped identifier.
fn render_callee_name<'s>(expr: &Expression<'_>, source: &'s str) -> Option<&'s str> {
    let call = match expr {
        Expression::CallExpression(c) => c,
        Expression::ParenthesizedExpression(p) => match p.expression {
            Expression::CallExpression(c) => c,
            _ => return None,
        },
        _ => return None,
    };
    match call.callee {
        Expression::Identifier(id) if id.escaped_name.is_none() => {
            let start = id.span.start as usize;
            Some(&source[start..start + id.name_len as usize])
        }
        _ => None,
    }
}

/// Whether a `{@render}` callee is *dynamic* (the oracle's
/// `binding?.kind !== 'normal'`): a local snippet is non-dynamic, a snippet prop
/// is dynamic. A non-standalone (dynamic) callee's fragment keeps the trailing
/// `<!---->` anchor even when the render is the sole child.
fn render_callee_dynamic(env: &EmitEnv<'_, '_>, name: &str) -> bool {
    !env.snippets.names.contains(name)
}

/// Emit `{@render callee(args)}` → `callee($$renderer, ...args)` (or
/// `callee?.($$renderer, …)` when optional), followed by a `<!---->` anchor
/// unless the enclosing fragment is standalone. The callee must resolve to a
/// local snippet or a snippet prop; anything else refuses. Arguments ride the
/// same value machinery as block tests (a bare derived read becomes `d()`).
fn emit_render_tag<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    tag: &'arena RenderTag<'arena>,
    out: &mut BodyBuilder<'arena>,
    is_standalone: bool,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    // "A `{@render}` holds a call expression" is a **parse-time** rule in the
    // oracle (`render_tag_invalid_expression`, raised while reading the tag) — so
    // it is decided on the RAW node, BEFORE type erasure. The distinction is
    // observable: `{@render (s as T)(x)}` is a call and compiles, while
    // `{@render (s(x) as T)}` and `{@render s(x)!}` are rejected even though
    // erasure would turn both into calls.
    if !render_expression_is_call(&tag.expression) {
        return Err(unsupported(Refusal::RenderTagUnsupportedCallee));
    }
    // The template borrow point: the whole `callee(args)` call is erased once, so
    // the callee classification and the arguments below both read the type-free
    // node (`{@render s<T>(x as U)}` → `s(x)`). Erasing a call yields a call, so
    // the shape settled above survives.
    let expression = env.erase(&tag.expression)?;
    let call = match expression {
        Expression::CallExpression(c) => c,
        Expression::ParenthesizedExpression(p) => match &p.expression {
            Expression::CallExpression(c) => c,
            _ => return Err(unsupported(Refusal::RenderTagUnsupportedCallee)),
        },
        _ => return Err(unsupported(Refusal::RenderTagUnsupportedCallee)),
    };
    // The callee must be a plain identifier resolving to a local snippet or a
    // snippet prop.
    let Some(name) = render_callee_name(expression, env.source) else {
        return Err(unsupported(Refusal::RenderTagUnsupportedCallee));
    };
    let is_snippet = env.snippets.names.contains(name);
    let is_prop = env.bindings.is_prop(name);
    if !is_snippet && !is_prop {
        return Err(unsupported(Refusal::RenderTagUnsupportedCallee));
    }

    // `callee($$renderer, ...args)`. Arguments go through the value machinery so
    // a bare derived read becomes `d()` and runes/mutations refuse.
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(Expression::Identifier(env.b.ident("$$renderer")));
    for arg in call.arguments {
        args.push(wrap_single(env, arg)?);
    }
    let render_call = env
        .b
        .call_of(call.callee, args.into_bump_slice(), call.optional);
    let span = render_call.span();
    let stmt = Statement::ExpressionStatement(ExpressionStatement {
        expression: render_call,
        span,
        is_directive: false,
    });
    out.push_statement(&mut env.b, arena, stmt);
    // A dynamic or non-sole render keeps the anchor so its output doesn't glue to
    // the surrounding fragment (the oracle's `empty_comment` push).
    if !is_standalone {
        out.push_text("<!---->");
    }
    Ok(())
}

/// Emit `{expr}` (escaped) or `{@html expr}` (raw) — the oracle's text-sequence
/// interpolation with its fold gate.
fn emit_expression_tag<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
    out: &mut BodyBuilder<'arena>,
    escape: bool,
) -> Result<(), CompileError> {
    // The template borrow point: erase TypeScript ONCE, then feed the erased node
    // to the guard AND the fold gate below (see `EmitEnv::erase`).
    let expr = env.erase(expr)?;
    // Guard FIRST (stray runes, non-bare derived reads, template mutations
    // refuse) — the evaluator must never fold an oracle-invalid expression.
    let wrapped = wrap_value_expr(env, expr)?;

    // The fold gate: a known evaluation folds into the static text.
    let evaluated = evaluate(expr, &env.value_scope(), env.source, 0)
        .map_err(|g| unsupported(Refusal::StaticEvalNotPortable(g.0)))?;
    if let Some(value) = evaluated.known_value() {
        if !escape {
            // A statically-known `{@html}` would fold through the oracle's html
            // path — not probed/ported, refuse rather than guess.
            return Err(unsupported(Refusal::HtmlTagStaticValue));
        }
        let text =
            stringify_value(value).map_err(|g| unsupported(Refusal::StaticFoldNotPortable(g.0)))?;
        out.push_text(&escape_template_text(&escape_html_text(&text)));
        return Ok(());
    }

    let call = if escape {
        env.b.member_call("$", "escape", wrapped)
    } else {
        env.b.member_call("$", "html", wrapped)
    };
    out.push_expr(call);
    Ok(())
}

/// Prepare a borrowed value expression for a synthetic call argument slot:
/// a bare read of a derived binding becomes `d()`; anything else is guarded
/// (stray runes, non-bare derived reads, and template mutations refuse) and
/// passed through borrowed.
fn wrap_value_expr<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<&'arena [Expression<'arena>], CompileError> {
    if let Expression::Identifier(id) = expr
        && id.escaped_name.is_none()
    {
        let start = id.span.start as usize;
        let name = &env.source[start..start + id.name_len as usize];
        if env.derived_names.contains(name) {
            let call = env.b.call_expr(expr, &[]);
            let call_alloc = env.b.arena.alloc(call);
            return Ok(std::slice::from_ref(call_alloc));
        }
    }
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let mut ctx = WalkCtx::new(env.source, &mut updated, &mut nested, &env.derived_names);
    walk_expression_guarded(expr, &mut ctx)?;
    if !updated.is_empty() {
        // A mutation here would postdate the binding analysis the fold already
        // consulted — refuse rather than fold stale.
        return Err(unsupported(Refusal::MutationInTemplateExpr));
    }
    Ok(std::slice::from_ref(expr))
}

/// The oracle's whitespace normalization (Svelte `clean_nodes`, whitespace
/// pass): boundary whitespace-only nodes dropped and edge-text runs trimmed,
/// then each text node's edge runs abutting a non-text node collapse to one
/// space (or nothing after a whitespace-ending text) — runs abutting `{expr}`
/// tags stay, interior whitespace stays. An all-collapsed `" "` text is dropped
/// entirely under the `select`/`table`-family parents.
fn normalize_whitespace(list: &mut Vec<CleanNode<'_>>, parent_name: Option<&str>) {
    // Boundary: drop whitespace-only text nodes, then trim the edge runs of a
    // surviving edge text node.
    while matches!(list.first(), Some(CleanNode::Text(t)) if is_ws_only(t)) {
        list.remove(0);
    }
    if let Some(CleanNode::Text(t)) = list.first_mut() {
        *t = replace_leading_ws(t, "");
    }
    while matches!(list.last(), Some(CleanNode::Text(t)) if is_ws_only(t)) {
        list.pop();
    }
    if let Some(CleanNode::Text(t)) = list.last_mut() {
        *t = replace_trailing_ws(t, "");
    }

    let can_remove_entirely =
        parent_name.is_some_and(|name| REMOVE_WS_ENTIRELY_PARENTS.contains(&name));

    // Inner pass: mutate in place reading the (already-mutated) previous
    // neighbor, mirroring the oracle's in-place iteration; drops applied after
    // so neighbors keep indexing the pre-drop list.
    let mut drop_flags = vec![false; list.len()];
    for i in 0..list.len() {
        let prev_is_expr = i > 0 && list[i - 1].is_expr();
        let prev_text_ends_ws = i > 0
            && matches!(&list[i - 1], CleanNode::Text(t) if t.chars().next_back().is_some_and(is_template_ws));
        let next_is_expr = list.get(i + 1).is_some_and(CleanNode::is_expr);
        let has_next = i + 1 < list.len();

        let CleanNode::Text(data) = &mut list[i] else {
            continue;
        };
        if i > 0 && !prev_is_expr {
            *data = replace_leading_ws(data, if prev_text_ends_ws { "" } else { " " });
        }
        if has_next && !next_is_expr {
            *data = replace_trailing_ws(data, " ");
        }
        if data.is_empty() || (data == " " && can_remove_entirely) {
            drop_flags[i] = true;
        }
    }
    let mut keep = drop_flags.iter();
    list.retain(|_| !*keep.next().unwrap_or(&false));
}

/// Emit one element's open tag, children, and close tag into the template.
fn emit_element<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    element: &'arena Element<'arena>,
    out: &mut BodyBuilder<'arena>,
    parent_ctx: &FragmentCtx<'_>,
    is_standalone: bool,
) -> Result<(), CompileError> {
    let name = env
        .b
        .interner
        .borrow()
        .resolve_infallible(element.name)
        .to_string();
    // A component (`<Foo>`, `<Foo.Bar>`) compiles to a call
    // (`Foo($$renderer, {…props})`), not static markup — route it to the
    // component emitter. Exhaustive over `ElementKind` so a new kind can't
    // silently fall through to the static-HTML path below.
    match element.kind {
        ElementKind::Html => {}
        ElementKind::Component => return emit_component(env, element, out, &name, is_standalone),
    }
    match name.as_str() {
        // Namespace-dependent whitespace/emission rules not implemented.
        "svg" | "math" => {
            return Err(unsupported(Refusal::ForeignNamespace {
                name: name.clone(),
            }));
        }
        // Template-level <script>/<style> have special semantics in the oracle.
        "script" | "style" => {
            return Err(unsupported(Refusal::TemplateLevelElement {
                name: name.clone(),
            }));
        }
        // The oracle compiles every <option> into `$$renderer.option(…)`
        // closure calls — static markup would be a divergent compile.
        "option" => {
            return Err(unsupported(Refusal::OptionElement));
        }
        // A populated <select>/<optgroup> gets a `<!>` anchor after its
        // children in the oracle's output (probe-verified; empty ones emit
        // statically and match, so only the populated shape refuses).
        "select" | "optgroup"
            if element
                .fragment
                .nodes
                .iter()
                .any(|n| !matches!(n, FragmentNode::Text(t) if t.is_ascii_ws_only)) =>
        {
            return Err(unsupported(Refusal::ElementWithChildren {
                name: name.clone(),
            }));
        }
        _ => {}
    }

    out.push_text(&format!("<{name}"));
    for attr_node in element.attributes {
        let AttributeNode::Attribute(attr) = attr_node else {
            return Err(unsupported(Refusal::NonPlainAttribute));
        };
        emit_attribute(env, attr, &name, out)?;
    }

    if tsv_html::is_void_element(&name) {
        // XHTML-compliant self-close, matching the oracle.
        out.push_text("/>");
        if !element.fragment.nodes.is_empty() {
            return Err(unsupported(Refusal::VoidElementChildren {
                name: name.clone(),
            }));
        }
        return Ok(());
    }
    out.push_text(">");
    emit_fragment(
        env,
        &element.fragment,
        out,
        FragmentCtx {
            mark_text_first: false,
            is_component_root: false,
            hoist_snippets: false,
            is_standalone,
            preserve_whitespace: parent_ctx.preserve_whitespace
                || name == "pre"
                || name == "textarea",
            parent_name: Some(&name),
        },
    )?;
    out.push_text(&format!("</{name}>"));
    Ok(())
}

/// Whether a component is *dynamic* — the oracle's `metadata.dynamic`
/// (`2-analyze/visitors/Component.js:14`): `binding !== null && (binding.kind !==
/// 'normal' || name.includes('.'))`. A dynamic component compiles to an
/// `if (expr) {…}` truthiness guard with hydration anchors, not a plain call —
/// refused in this slice.
///
/// - A member component (`<Foo.Bar>`) is always dynamic.
/// - A block-local name (each item/index, `{:then}` value, `{@const}`) resolves
///   through an overlay to a non-`normal` binding → dynamic.
/// - A top-level `prop`/`$derived`/`$state` binding → dynamic. A plain
///   declaration/import (`normal`) or an unresolved global (`binding === null`)
///   is **not** dynamic.
fn component_dynamic(env: &EmitEnv<'_, '_>, name: &str) -> bool {
    if name.contains('.') {
        return true;
    }
    if env
        .overlays
        .iter()
        .any(|overlay| overlay.contains_key(name))
    {
        return true;
    }
    match env.bindings.get(name) {
        None => false,
        Some(binding) => match binding.kind {
            BindingKind::Prop | BindingKind::Derived => true,
            BindingKind::Normal | BindingKind::Opaque => env.state_names.contains(name),
        },
    }
}

/// Whether a sole fragment child is a standalone-eligible component (the oracle's
/// `clean_nodes` `is_standalone`: a non-dynamic `Component` with no
/// `--custom-property` attribute — `hmr` is always off here). When true its call
/// reuses the enclosing block's anchor and emits no trailing `<!---->`.
fn component_is_standalone_eligible(env: &EmitEnv<'_, '_>, element: &Element<'_>) -> bool {
    if element.kind != ElementKind::Component {
        return false;
    }
    let name = {
        let interner = env.b.interner.borrow();
        interner.resolve_infallible(element.name).to_string()
    };
    if component_dynamic(env, &name) {
        return false;
    }
    !element.attributes.iter().any(|attr_node| {
        let AttributeNode::Attribute(attr) = attr_node else {
            return false;
        };
        let interner = env.b.interner.borrow();
        interner.resolve_infallible(attr.name).starts_with("--")
    })
}

/// The component-children analysis product: the synthetic props to append after
/// the attribute props, plus the snippet-prop function declarations that go into
/// the component's wrapping block.
struct ChildrenPlan<'arena> {
    /// `function name($$renderer, …) { … }` declarations (source order), placed in
    /// the component's wrapping block so the snippet props can reference them.
    snippet_functions: Vec<Statement<'arena>>,
    /// Snippet prop names (source order) — each emits a `{ name }` shorthand prop.
    snippet_props: Vec<String>,
    /// `$$slots` entries (slot keys, source order): snippet slots, then `default`.
    slot_keys: Vec<String>,
    /// The default-slot children fragment (direct `{#snippet}` children filtered
    /// out) → the implicit `children: ($$renderer) => { … }` arrow, if any.
    default_children: Option<Fragment<'arena>>,
}

impl ChildrenPlan<'_> {
    /// Whether the plan contributes any synthetic props (a snippet prop, the
    /// `children` arrow, or `$$slots`) — `slot_keys` is non-empty exactly then.
    fn has_content(&self) -> bool {
        !self.slot_keys.is_empty()
    }
}

/// Plan a component's children: build the `{#snippet}` prop functions (in source
/// order) and the synthetic prop shape, refusing the deferred cases — a `slot="…"`
/// child (named slot) or a `children` prop alongside default children (the
/// oracle's `$$slots.default` divergence). A `{#snippet}` child named `children`
/// keeps the `children` prop name but a `default` slot key (the oracle's rename).
fn plan_component_children<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    element: &'arena Element<'arena>,
    name: &str,
) -> Result<ChildrenPlan<'arena>, CompileError> {
    let arena = env.b.arena;
    let mut snippet_functions = Vec::new();
    let mut snippet_props = Vec::new();
    let mut slot_keys = Vec::new();
    let mut has_default = false;
    for node in element.fragment.nodes {
        match node {
            FragmentNode::SnippetBlock(snippet) => {
                let (func, snippet_name) = build_snippet_function(env, snippet)?;
                snippet_functions.push(func);
                // The oracle serializes a snippet named `children` under the
                // `default` slot key, but the prop keeps the `children` name.
                let slot_key = if snippet_name == "children" {
                    "default".to_string()
                } else {
                    snippet_name.clone()
                };
                snippet_props.push(snippet_name);
                slot_keys.push(slot_key);
            }
            FragmentNode::Comment(_) => {}
            FragmentNode::Text(text) if text.is_ascii_ws_only => {}
            FragmentNode::Element(child) if child_slot_attribute(env, child) => {
                return Err(unsupported(Refusal::ComponentNamedSlot {
                    name: name.to_string(),
                }));
            }
            _ => has_default = true,
        }
    }
    // A `children` prop AND default children route through `$$slots.default` with
    // a `children` error in the oracle — a divergent shape; refuse.
    if has_default && component_has_named_attribute(env, element, "children") {
        return Err(unsupported(Refusal::ComponentChildrenPropConflict {
            name: name.to_string(),
        }));
    }
    let default_children = if has_default {
        // The `children` arrow sees only the default-slot children — direct
        // `{#snippet}` children live in the wrapping block, not the arrow body.
        let mut nodes: BumpVec<'arena, FragmentNode<'arena>> = BumpVec::new_in(arena);
        for node in element.fragment.nodes {
            if !matches!(node, FragmentNode::SnippetBlock(_)) {
                nodes.push(node.clone());
            }
        }
        slot_keys.push("default".to_string());
        Some(Fragment {
            nodes: nodes.into_bump_slice(),
        })
    } else {
        None
    };
    Ok(ChildrenPlan {
        snippet_functions,
        snippet_props,
        slot_keys,
        default_children,
    })
}

/// Whether a component child element carries a `slot="…"` attribute (a named
/// slot).
fn child_slot_attribute(env: &EmitEnv<'_, '_>, element: &Element<'_>) -> bool {
    component_has_named_attribute(env, element, "slot")
}

/// Whether an element carries a plain attribute with the given (case-sensitive)
/// name.
fn component_has_named_attribute(env: &EmitEnv<'_, '_>, element: &Element<'_>, name: &str) -> bool {
    element.attributes.iter().any(|attr_node| {
        let AttributeNode::Attribute(attr) = attr_node else {
            return false;
        };
        let interner = env.b.interner.borrow();
        interner.resolve_infallible(attr.name) == name
    })
}

/// Whether `name` matches the ECMAScript identifier grammar Svelte's `b.key` uses
/// (`regex_is_valid_identifier`, `/^[a-zA-Z_$][a-zA-Z_$0-9]*$/`) — an identifier
/// key, otherwise a string-literal key.
fn is_valid_js_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Emit `Name($$renderer, props)` for a component invocation (`<Foo … />`),
/// followed by a trailing `<!---->` anchor unless the enclosing fragment is
/// standalone (the oracle's `empty_comment` push). Named-snippet children wrap
/// the call in a bare `{ function …; Name(…); }` block. Dynamic components, named
/// slots, `--custom-property` attributes, and directives are refused — see the
/// individual refusal sites.
fn emit_component<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    element: &'arena Element<'arena>,
    out: &mut BodyBuilder<'arena>,
    name: &str,
    is_standalone: bool,
) -> Result<(), CompileError> {
    let arena = env.b.arena;
    // A dynamic component (member / reactive binding) compiles to the truthiness
    // guard — a later slice.
    if component_dynamic(env, name) {
        return Err(unsupported(Refusal::DynamicComponent {
            name: name.to_string(),
        }));
    }

    // Children plan: the `{#snippet}` prop functions (for the wrapping block) and
    // the synthetic props (snippet props, the `children` arrow, `$$slots`). Built
    // before the props so the snippet functions mint before the props reference
    // them.
    let plan = plan_component_children(env, element, name)?;

    // Build the props/spreads expression (a plain object, or `$.spread_props`),
    // appending the synthetic children props from the plan.
    let props_expr = build_component_props(env, element, name, &plan)?;

    // `Name($$renderer, props)`. The callee is the component reference (a plain
    // identifier — member components refuse above).
    let callee = env.b.ident_expr(name);
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(Expression::Identifier(env.b.ident("$$renderer")));
    args.push(props_expr);
    let call = env.b.call_of(callee, args.into_bump_slice(), false);
    let span = call.span();
    let call_stmt = Statement::ExpressionStatement(ExpressionStatement {
        expression: call,
        span,
        is_directive: false,
    });

    // Named-snippet children hoist their `function` declarations into a bare block
    // wrapping the call, so the snippet props resolve (the oracle's
    // `b.block([...snippet_declarations, statement])`).
    let stmt = if plan.snippet_functions.is_empty() {
        call_stmt
    } else {
        let mut block_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        for func in plan.snippet_functions {
            block_body.push(func);
        }
        block_body.push(call_stmt);
        let block_span = env.b.here();
        Statement::BlockStatement(BlockStatement {
            body: block_body.into_bump_slice(),
            span: block_span,
        })
    };
    out.push_statement(&mut env.b, arena, stmt);

    // A non-standalone component keeps the `<!---->` anchor so its output doesn't
    // fuse with the surrounding fragment.
    if !is_standalone {
        out.push_text("<!---->");
    }
    Ok(())
}

/// A group of component attributes: consecutive plain attributes accumulate into
/// one object literal; a `{...spread}` starts a new group (the oracle's
/// `props_and_spreads` grouping in `shared/component.js`).
enum PropGroup<'a, 'arena> {
    Props(Vec<&'a Attribute<'arena>>),
    Spread(&'a Expression<'arena>),
}

/// Build the component call's props argument: a plain object `{ … }` when there
/// are no spreads (or a single leading props group), otherwise
/// `$.spread_props([ … ])` interleaving objects and spread expressions in source
/// order. Refuses `--custom-property` attributes, `bind:`, and other directives.
fn build_component_props<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    element: &'arena Element<'arena>,
    name: &str,
    plan: &ChildrenPlan<'arena>,
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    // The synthetic children props (snippet props, the `children` arrow,
    // `$$slots`) go into the last props group, or a new one after a trailing
    // spread.
    let synthetic = plan.has_content().then_some(plan);
    let mut groups: Vec<PropGroup<'_, 'arena>> = Vec::new();
    for attr_node in element.attributes {
        match attr_node {
            AttributeNode::Attribute(attr) => {
                let attr_name = {
                    let interner = env.b.interner.borrow();
                    interner.resolve_infallible(attr.name).to_string()
                };
                // A `--custom-property` attribute takes the oracle's `$.css_props`
                // path — a later slice.
                if attr_name.starts_with("--") {
                    return Err(unsupported(Refusal::ComponentCustomProperty {
                        name: name.to_string(),
                    }));
                }
                match groups.last_mut() {
                    Some(PropGroup::Props(props)) => props.push(attr),
                    _ => groups.push(PropGroup::Props(vec![attr])),
                }
            }
            AttributeNode::SpreadAttribute(spread) => {
                groups.push(PropGroup::Spread(&spread.expression));
            }
            AttributeNode::BindDirective(_) => {
                return Err(unsupported(Refusal::ComponentBindDirective {
                    name: name.to_string(),
                }));
            }
            _ => {
                return Err(unsupported(Refusal::ComponentDirective {
                    name: name.to_string(),
                }));
            }
        }
    }

    // The oracle emits a plain object when there are no spreads (no groups, or a
    // single props group); otherwise `$.spread_props([...])`. The synthetic props
    // append to the last props group, or a new one when the last group is a spread
    // (the oracle's `push_prop`).
    let single_object =
        groups.is_empty() || (groups.len() == 1 && matches!(groups[0], PropGroup::Props(_)));
    if single_object {
        let attrs: &[&Attribute<'arena>] = match groups.first() {
            Some(PropGroup::Props(props)) => props,
            _ => &[],
        };
        return build_props_object(env, attrs, synthetic);
    }

    // `$.spread_props([ obj_or_spread, … ])`. Mint the brackets around the
    // element construction so the array span encloses the minted object spans.
    let last_is_props = matches!(groups.last(), Some(PropGroup::Props(_)));
    let lbracket = env.b.mint("[").start;
    let mut elements: BumpVec<'arena, Option<Expression<'arena>>> = BumpVec::new_in(arena);
    let group_count = groups.len();
    for (i, group) in groups.iter().enumerate() {
        let element_expr = match group {
            PropGroup::Props(props) => {
                // The synthetic props join the last props group.
                let syn = if i + 1 == group_count {
                    synthetic
                } else {
                    None
                };
                build_props_object(env, props, syn)?
            }
            PropGroup::Spread(expr) => {
                // The template borrow point (`<Foo {...(o as any)} />`).
                let expr = env.erase(expr)?;
                wrap_value_expr(env, expr)?[0].clone()
            }
        };
        elements.push(Some(element_expr));
    }
    // A trailing spread with synthetic props needs its own props object appended.
    if !last_is_props && let Some(plan) = synthetic {
        elements.push(Some(build_props_object(env, &[], Some(plan))?));
    }
    let rbracket = env.b.mint("]").end;
    let array = Expression::ArrayExpression(ArrayExpression {
        elements: elements.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(lbracket, rbracket),
    });
    let array_alloc = arena.alloc(array);
    Ok(env
        .b
        .member_call("$", "spread_props", std::slice::from_ref(array_alloc)))
}

/// Build a plain object literal `{ … }` from a run of component attributes. The
/// braces are minted around the property construction so the object span encloses
/// the (appendix) key spans (the object printer reads its own span region for the
/// expansion decision — all appendix, no newlines, so it collapses when it fits).
fn build_props_object<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attrs: &[&'arena Attribute<'arena>],
    synthetic: Option<&ChildrenPlan<'arena>>,
) -> Result<Expression<'arena>, CompileError> {
    let arena = env.b.arena;
    let obrace = env.b.mint("{").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for attr in attrs {
        properties.push(ObjectProperty::Property(build_component_property(
            env, attr,
        )?));
    }
    // The synthetic children props, in the oracle's order: snippet props (source
    // order), then the implicit `children` arrow, then `$$slots`.
    if let Some(plan) = synthetic {
        for snippet_name in &plan.snippet_props {
            properties.push(ObjectProperty::Property(build_snippet_prop(
                env,
                snippet_name,
            )));
        }
        if let Some(fragment) = &plan.default_children {
            properties.push(ObjectProperty::Property(build_children_prop(
                env, fragment,
            )?));
        }
        properties.push(ObjectProperty::Property(build_slots_prop(
            env,
            &plan.slot_keys,
        )));
    }
    let cbrace = env.b.mint("}").end;
    Ok(Expression::ObjectExpression(ObjectExpression {
        properties: properties.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    }))
}

/// A `{ name }` shorthand prop for a named-snippet child — the value references
/// the `function name(…)` declaration in the component's wrapping block.
fn build_snippet_prop<'arena>(env: &mut EmitEnv<'arena, '_>, name: &str) -> Property<'arena> {
    let key = env.b.ident(name);
    let key_span = key.span;
    let value = env.b.ident(name);
    Property {
        key: Expression::Identifier(key),
        value: Expression::Identifier(value),
        kind: PropertyKind::Init,
        shorthand: true,
        computed: false,
        method: false,
        span: key_span,
    }
}

/// The implicit `children` prop for a component's default-slot children:
/// `children: ($$renderer) => { …body… }`. The body reuses the fragment
/// machinery (text-first eligible, per the oracle's `is_text_first` Component
/// parent). The key is minted first so the (key-only) property span stays forward.
fn build_children_prop<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    fragment: &Fragment<'arena>,
) -> Result<Property<'arena>, CompileError> {
    let arena = env.b.arena;
    let key = env.b.ident("children");
    let key_span = key.span;
    let body = emit_child_body(env, fragment, &[], true, false, HashMap::new())?;
    let renderer_param = Expression::Identifier(env.b.ident("$$renderer"));
    let params = std::slice::from_ref(arena.alloc(renderer_param));
    let block_span = env.b.here();
    let arrow = env.b.arrow_block(params, body, block_span);
    Ok(Property {
        key: Expression::Identifier(key),
        value: arrow,
        kind: PropertyKind::Init,
        shorthand: false,
        computed: false,
        method: false,
        span: key_span,
    })
}

/// The `$$slots: { key1: true, … }` prop that accompanies component children —
/// one `true` entry per named-snippet slot plus `default` for default children
/// (slot names are always valid identifiers). Named-slot arrow values would live
/// here too, but named slots are refused.
fn build_slots_prop<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    slot_keys: &[String],
) -> Property<'arena> {
    let arena = env.b.arena;
    let key = env.b.ident("$$slots");
    let key_span = key.span;
    let obrace = env.b.mint("{").start;
    let mut inner_props: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for slot_key in slot_keys {
        let entry_key = env.b.ident(slot_key);
        let entry_key_span = entry_key.span;
        let entry_val = env.b.true_literal();
        inner_props.push(ObjectProperty::Property(Property {
            key: Expression::Identifier(entry_key),
            value: entry_val,
            kind: PropertyKind::Init,
            shorthand: false,
            computed: false,
            method: false,
            span: entry_key_span,
        }));
    }
    let cbrace = env.b.mint("}").end;
    let inner = Expression::ObjectExpression(ObjectExpression {
        properties: inner_props.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    });
    Property {
        key: Expression::Identifier(key),
        value: inner,
        kind: PropertyKind::Init,
        shorthand: false,
        computed: false,
        method: false,
        span: key_span,
    }
}

/// Build one `key: value` object property from a component attribute. The key is
/// an identifier when it matches the identifier grammar (else a string literal);
/// `shorthand` is set when the key is an identifier and the value is the plain
/// identifier of the same name (`{ n: n }` prints as `{ n }`). The key is minted
/// before the value, so the (key-only) property span stays forward and in the
/// appendix; the value prints from its own span.
fn build_component_property<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attr: &'arena Attribute<'arena>,
) -> Result<Property<'arena>, CompileError> {
    let name = {
        let interner = env.b.interner.borrow();
        interner.resolve_infallible(attr.name).to_string()
    };
    let key_is_ident = is_valid_js_identifier(&name);
    let key = if key_is_ident {
        Expression::Identifier(env.b.ident(&name))
    } else {
        env.b.string_literal_expr(&name)
    };
    let key_span = key.span();
    let value = build_prop_value(env, attr)?;
    let shorthand = key_is_ident
        && matches!(&value, Expression::Identifier(id)
            if plain_identifier_name(id, env.source).as_deref() == Some(name.as_str()));
    Ok(Property {
        key,
        value,
        kind: PropertyKind::Init,
        shorthand,
        computed: false,
        method: false,
        span: key_span,
    })
}

/// Build a component attribute's prop value:
///
/// - a boolean attribute → `true`;
/// - a single static text value → the *decoded* data as a string literal (no
///   HTML escaping, no trim — the oracle's `is_component` branch of
///   `build_attribute_value`);
/// - a single expression value → guarded, bare-derived → `d()`, passed through
///   with **no fold** (the single-chunk component path doesn't evaluate);
/// - a mixed text+expression value → a template literal (or a folded string
///   literal when every part is statically known).
fn build_prop_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attr: &'arena Attribute<'arena>,
) -> Result<Expression<'arena>, CompileError> {
    let Some(values) = attr.value else {
        return Ok(env.b.true_literal());
    };
    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            Ok(env.b.string_literal_expr(&decoded))
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // The template borrow point. The erased node also decides the caller's
            // `{ n }` shorthand test — `<Foo n={n as T} />` is `{ n }` to the oracle.
            let expr = env.erase(&tag.expression)?;
            let wrapped = wrap_value_expr(env, expr)?;
            Ok(wrapped[0].clone())
        }
        _ => build_component_mixed_value(env, values),
    }
}

/// Build a mixed text+expression component attribute value. Unlike the element
/// mixed-attribute path there is no whitespace trim, no HTML escaping, and no
/// `$.attr*` wrapper — the oracle's component `build_attribute_value` returns the
/// bare value: a folded string literal when every part is statically known, else
/// a template literal with `$.stringify(expr)` interpolations (omitted when the
/// evaluator proves a defined string).
fn build_component_mixed_value<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    values: &'arena [AttributeValue<'arena>],
) -> Result<Expression<'arena>, CompileError> {
    let mut texts: Vec<String> = vec![String::new()];
    // The unescaped folded value in parallel — consumed only when every part folds.
    let mut raw = String::new();
    let mut exprs: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    for value in values {
        match value {
            AttributeValue::Text(text) => {
                let decoded = text.data(env.source);
                raw.push_str(&decoded);
                #[allow(clippy::unwrap_used)]
                texts
                    .last_mut()
                    .unwrap()
                    .push_str(&escape_template_text(&decoded));
            }
            AttributeValue::ExpressionTag(tag) => {
                // The template borrow point: erase once, then guard AND fold the
                // erased node (the fold gate is the silent-divergence trap).
                let expr = env.erase(&tag.expression)?;
                // Guard first — never fold an oracle-invalid expression.
                let wrapped = wrap_value_expr(env, expr)?;
                let evaluated = evaluate(expr, &env.value_scope(), env.source, 0)
                    .map_err(|g| unsupported(Refusal::StaticEvalNotPortable(g.0)))?;
                if let Some(value) = evaluated.known_value() {
                    let text = stringify_value(value)
                        .map_err(|g| unsupported(Refusal::StaticFoldNotPortable(g.0)))?;
                    raw.push_str(&text);
                    #[allow(clippy::unwrap_used)]
                    texts
                        .last_mut()
                        .unwrap()
                        .push_str(&escape_template_text(&text));
                    continue;
                }
                let piece = if evaluated.is_defined_string() {
                    wrapped[0].clone()
                } else {
                    env.b.member_call("$", "stringify", wrapped)
                };
                exprs.push(piece);
                texts.push(String::new());
            }
        }
    }

    if exprs.is_empty() {
        return Ok(env.b.string_literal_expr(&raw));
    }
    Ok(env.b.template_literal(&texts, exprs.into_bump_slice()))
}

/// Recursively test whether a fragment contains a component (`<Foo … />`) — the
/// comments+component refusal gate.
fn fragment_has_component(fragment: &Fragment<'_>) -> bool {
    fragment.nodes.iter().any(|node| match node {
        FragmentNode::Element(element) => {
            element.kind == ElementKind::Component || fragment_has_component(&element.fragment)
        }
        FragmentNode::SpecialElement(se) => fragment_has_component(&se.fragment),
        FragmentNode::IfBlock(b) => {
            fragment_has_component(&b.consequent)
                || b.alternate.as_ref().is_some_and(fragment_has_component)
        }
        FragmentNode::EachBlock(b) => {
            fragment_has_component(&b.body)
                || b.fallback.as_ref().is_some_and(fragment_has_component)
        }
        FragmentNode::AwaitBlock(b) => [&b.pending, &b.then, &b.catch]
            .into_iter()
            .flatten()
            .any(fragment_has_component),
        FragmentNode::KeyBlock(b) => fragment_has_component(&b.fragment),
        FragmentNode::SnippetBlock(s) => fragment_has_component(&s.body),
        _ => false,
    })
}

/// Emit one plain attribute. Static text values inline (with entity decoding,
/// attribute escaping, `class`/`style` whitespace collapse, and the scope hash
/// on matched classes); dynamic and mixed values emit the oracle's runtime
/// calls (`$.attr` / `$.attr_class` / `$.attr_style`).
fn emit_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    attr: &'arena Attribute<'arena>,
    element_name: &str,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // The oracle lowercases attribute names outside foreign namespaces (svg
    // refuses above) — at EMISSION only. The event-handler decision below tests
    // the RAW authored name, so both are kept.
    let raw_name = env
        .b
        .interner
        .borrow()
        .resolve_infallible(attr.name)
        .to_string();
    let name = raw_name.to_ascii_lowercase();

    // `value` on <textarea> becomes child content, on <select> it is omitted
    // with select_value bookkeeping — neither shape is implemented.
    if name == "value" && (element_name == "textarea" || element_name == "select") {
        return Err(unsupported(Refusal::ValueAttribute {
            name: element_name.to_string(),
        }));
    }

    let Some(values) = attr.value else {
        // Boolean attribute: the oracle emits `name=""`.
        out.push_text(&escape_template_text(&format!(" {name}=\"\"")));
        return Ok(());
    };

    match values {
        [AttributeValue::Text(text)] => {
            let decoded = text.data(env.source);
            let mut value = if name == "class" || name == "style" {
                collapse_attr_whitespace(&decoded)
            } else {
                decoded.into_owned()
            };
            // A string-valued `class` that collapses+trims to empty is dropped
            // entirely (oracle-probed: `class=""` and `class="  "` emit no
            // attribute). Class-specific and static-path-specific: a bare
            // `class` (boolean form, handled above) keeps `class=""`, empty
            // `style`/`id` stay, and a *folded* mixed class keeps `class=""`
            // (see `emit_mixed_attribute`).
            if name == "class" && value.is_empty() {
                return Ok(());
            }
            if name == "class"
                && let Some(scope) = &env.scope
            {
                let mut matched = false;
                for class in value.split_ascii_whitespace() {
                    if scope.class_names.contains(class) {
                        env.matched_classes.insert(class.to_string());
                        matched = true;
                    }
                }
                if matched {
                    value.push(' ');
                    value.push_str(SCOPE_HASH_CLASS);
                }
            }
            out.push_text(&escape_template_text(&format!(
                " {name}=\"{}\"",
                escape_html_attr(&value)
            )));
            Ok(())
        }
        [AttributeValue::ExpressionTag(tag)] => {
            // An `on`-prefixed single-expression attribute is an event handler —
            // tested on the RAW authored name, exactly like the oracle
            // (`is_event_attribute`, server `element.js:71`, runs before any
            // lowercasing): `onClick` drops, but `ONCLICK`/`oNclick` are NOT
            // events and emit as regular `$.attr('onclick', …)` attributes
            // (probe-verified). A dropped handler's expression still feeds
            // `needs_context` (walked up front in `needs_context.rs`), so a
            // `new`/prop-rooted member or call inside it still forces the
            // wrapper — only the attribute markup is dropped. Raw `onload`/
            // `onerror` (exact match — `onLoad` on `<img>` is a plain drop,
            // probe-verified) on a load-error element are the exception (the
            // oracle injects an `on{name}="this.__e=event"` capture attribute),
            // refused for now.
            if raw_name.starts_with("on") {
                if (raw_name == "onload" || raw_name == "onerror")
                    && is_load_error_element(element_name)
                {
                    return Err(unsupported(Refusal::EventCaptureAttribute { name }));
                }
                // Dropped, but still guarded: a misplaced rune inside a handler is
                // an oracle analysis-phase error, not an emission one.
                return guard_dropped(env, &tag.expression);
            }
            // Quoted (`class="{a}"`) vs bare (`class={a}`): the oracle's AST
            // represents the quoted form as a one-chunk ARRAY and the bare form
            // as a plain ExpressionTag (the wire writer's `preceded_by_quote`
            // discriminant) — the split the class `$.clsx` rule keys on.
            let quoted = preceded_by_quote(env.source, tag.span.start);
            emit_dynamic_attribute(env, &name, &tag.expression, quoted, out)
        }
        _ => {
            // A mixed-value attribute whose RAW name starts with `on` is an
            // event attribute the oracle rejects as input
            // (`attribute_invalid_event_handler`) — refuse rather than guess.
            // `ONCLICK="a {h}"` is NOT an event (raw test) and emits through the
            // normal mixed path (probe-verified).
            if raw_name.starts_with("on") {
                return Err(unsupported(Refusal::EventAttribute { name }));
            }
            emit_mixed_attribute(env, &name, values, out)
        }
    }
}

/// Whether the byte before `pos` is a quote — the same discriminant the wire
/// writer uses to emit a quoted single-expression attribute value as an array.
fn preceded_by_quote(source: &str, pos: u32) -> bool {
    matches!(
        (pos as usize)
            .checked_sub(1)
            .and_then(|i| source.as_bytes().get(i)),
        Some(b'"' | b'\'')
    )
}

/// The oracle's `needs_clsx` rule (`2-analyze/visitors/Attribute.js`): only a
/// bare `class={expr}` wraps in `$.clsx`, and only when the expression is not a
/// `Literal`, `TemplateLiteral`, or ESTree `BinaryExpression`. tsv's internal
/// AST folds logical operators into `BinaryExpression`, but ESTree types them
/// `LogicalExpression` (`&&`/`||`/`??` DO wrap — oracle-probed), so the
/// exclusion is arithmetic/comparison binaries only. The terminal arm mirrors
/// the oracle's own negative-list rule: everything else wraps.
fn class_needs_clsx(expr: &Expression<'_>, quoted: bool) -> bool {
    if quoted {
        return false;
    }
    match expr {
        Expression::Literal(_) | Expression::TemplateLiteral(_) => false,
        Expression::BinaryExpression(b) => b.operator.is_logical(),
        _ => true,
    }
}

/// `class`/`style` value whitespace collapse (`[ \t\n\r\f]+` → one space, then
/// trim) — the oracle's `WHITESPACE_INSENSITIVE_ATTRIBUTES` handling.
fn collapse_attr_whitespace(decoded: &str) -> String {
    let mut collapsed = String::with_capacity(decoded.len());
    let mut in_ws = false;
    for c in decoded.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c') {
            in_ws = true;
        } else {
            if in_ws && !collapsed.is_empty() {
                collapsed.push(' ');
            }
            in_ws = false;
            collapsed.push(c);
        }
    }
    collapsed
}

/// A single-expression attribute value: `title={expr}` (or quoted,
/// `title="{expr}"` — `quoted` carries the distinction, which only the class
/// `$.clsx` rule reads).
fn emit_dynamic_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    expr: &'arena Expression<'arena>,
    quoted: bool,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // Event handlers (`on*` single-expression attributes) are dropped in
    // `emit_attribute` before dispatch, so they never reach here.
    // The `$.attr` family interleaves minted (appendix) and borrowed (host)
    // argument spans; with host comments present their windows would sweep.
    if env.has_comments {
        return Err(unsupported(Refusal::CommentsAlongsideExprAttributes));
    }
    // The template borrow point. Both shape predicates below read the node's
    // variant, so they must see the ERASED one: `class={'a' as string}` is a
    // string literal to the oracle (inline-literal path), and a `TSAsExpression`
    // left in place would take the `$.clsx` branch instead.
    let expr = env.erase(expr)?;
    // A string-literal expression value takes the oracle's inline-literal path
    // (pre-escaped static emission) — refuse rather than guess its edge rules.
    if matches!(expr, Expression::Literal(lit)
        if matches!(lit.value, LiteralValue::String(_)))
    {
        return Err(unsupported(Refusal::StringLiteralExprAttribute));
    }

    let wrapped = wrap_value_expr(env, expr)?;
    let call = match name {
        // Dynamic class/style interact with CSS scoping (hash argument,
        // pruning) — supported only on unstyled components.
        "class" => {
            if env.scope.is_some() {
                return Err(unsupported(Refusal::DynamicClassOnStyled));
            }
            if class_needs_clsx(expr, quoted) {
                let clsx = env.b.member_call("$", "clsx", wrapped);
                let clsx_alloc = env.b.arena.alloc(clsx);
                env.b
                    .member_call("$", "attr_class", std::slice::from_ref(clsx_alloc))
            } else {
                env.b
                    .member_call("$", "attr_class", std::slice::from_ref(&wrapped[0]))
            }
        }
        "style" => {
            if env.scope.is_some() {
                return Err(unsupported(Refusal::DynamicStyleOnStyled));
            }
            env.b.member_call("$", "attr_style", wrapped)
        }
        _ => {
            let mut args = BumpVec::new_in(env.b.arena);
            args.push(env.b.string_literal_expr(name));
            args.push(wrapped[0].clone());
            if DOM_BOOLEAN_ATTRIBUTES.contains(&name) {
                args.push(env.b.true_literal());
            }
            env.b.member_call("$", "attr", args.into_bump_slice())
        }
    };
    out.push_expr(call);
    Ok(())
}

/// A mixed text+expression attribute value: `title="t {a} u"` — an attribute
/// template literal with `$.stringify(expr)` interpolations (omitted when the
/// oracle's evaluator proves a defined string), folded where known.
fn emit_mixed_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    values: &'arena [AttributeValue<'arena>],
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // Event attributes (RAW name starting `on`) are refused by the dispatch in
    // `emit_attribute` before this is reached.
    if env.has_comments {
        return Err(unsupported(Refusal::CommentsAlongsideExprAttributes));
    }
    if (name == "class" || name == "style") && env.scope.is_some() {
        return Err(unsupported(Refusal::InterpolatedAttrOnStyled {
            name: name.to_string(),
        }));
    }
    let trim_whitespace = name == "class" || name == "style";

    let mut texts: Vec<String> = vec![String::new()];
    // The unescaped folded value, in parallel — consumed only when every part
    // folds statically (the full-fold static emission below).
    let mut raw = String::new();
    let mut exprs: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(env.b.arena);
    for value in values {
        match value {
            AttributeValue::Text(text) => {
                let decoded = text.data(env.source);
                let chunk = if trim_whitespace {
                    // Runs collapse but edges are NOT trimmed per-chunk (the
                    // oracle's replace() without trim in the template path).
                    collapse_runs_no_trim(&decoded)
                } else {
                    decoded.into_owned()
                };
                raw.push_str(&chunk);
                // Attribute templates carry no HTML escaping — the runtime
                // escapes; only template metachars are escaped here.
                #[allow(clippy::unwrap_used)]
                texts
                    .last_mut()
                    .unwrap()
                    .push_str(&escape_template_text(&chunk));
            }
            AttributeValue::ExpressionTag(tag) => {
                // The template borrow point: erase once, then guard AND fold the
                // erased node (the fold gate is the silent-divergence trap).
                let expr = env.erase(&tag.expression)?;
                // Guard first — never fold an oracle-invalid expression.
                let wrapped = wrap_value_expr(env, expr)?;
                let evaluated = evaluate(expr, &env.value_scope(), env.source, 0)
                    .map_err(|g| unsupported(Refusal::StaticEvalNotPortable(g.0)))?;
                if let Some(value) = evaluated.known_value() {
                    // Folds into the quasi — plain `(value ?? '') + ''`, no
                    // HTML escaping in the template-value path.
                    let text = stringify_value(value)
                        .map_err(|g| unsupported(Refusal::StaticFoldNotPortable(g.0)))?;
                    raw.push_str(&text);
                    #[allow(clippy::unwrap_used)]
                    texts
                        .last_mut()
                        .unwrap()
                        .push_str(&escape_template_text(&text));
                    continue;
                }
                let piece = if evaluated.is_defined_string() {
                    wrapped[0].clone()
                } else {
                    env.b.member_call("$", "stringify", wrapped)
                };
                exprs.push(piece);
                texts.push(String::new());
            }
        }
    }

    if exprs.is_empty() {
        // Every part folded statically — the oracle emits a *static* attribute
        // (oracle-probed rules): attr-escape `[&"<]`, folded value verbatim (no
        // trim, no empty-class drop, boolean attributes keep the folded value,
        // null/undefined already stringified to '' by the fold above). Only the
        // chunk-array path folds; a single-expression attribute never does
        // (`emit_dynamic_attribute`).
        out.push_text(&escape_template_text(&format!(
            " {name}=\"{}\"",
            escape_html_attr(&raw)
        )));
        return Ok(());
    }

    let template = env.b.template_literal(&texts, exprs.into_bump_slice());
    let template_alloc = env.b.arena.alloc(template);
    let call = match name {
        "class" => env
            .b
            .member_call("$", "attr_class", std::slice::from_ref(template_alloc)),
        "style" => env
            .b
            .member_call("$", "attr_style", std::slice::from_ref(template_alloc)),
        _ => {
            let mut args = BumpVec::new_in(env.b.arena);
            args.push(env.b.string_literal_expr(name));
            args.push(template_alloc.clone());
            if DOM_BOOLEAN_ATTRIBUTES.contains(&name) {
                args.push(env.b.true_literal());
            }
            env.b.member_call("$", "attr", args.into_bump_slice())
        }
    };
    out.push_expr(call);
    Ok(())
}

/// Collapse `[ \t\n\r\f]+` runs to one space without trimming (the mixed-value
/// `class`/`style` chunk rule).
fn collapse_runs_no_trim(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_ws = false;
    for c in s.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0c') {
            in_ws = true;
        } else {
            if in_ws {
                out.push(' ');
            }
            in_ws = false;
            out.push(c);
        }
    }
    if in_ws {
        out.push(' ');
    }
    out
}

fn fragment_node_kind(node: &FragmentNode<'_>) -> &'static str {
    match node {
        FragmentNode::Element(_) => "element",
        FragmentNode::SpecialElement(_) => "special element",
        FragmentNode::ExpressionTag(_) => "expression tag",
        FragmentNode::Text(_) => "text",
        FragmentNode::Comment(_) => "html comment",
        FragmentNode::IfBlock(_) => "{#if} block",
        FragmentNode::EachBlock(_) => "{#each} block",
        FragmentNode::AwaitBlock(_) => "{#await} block",
        FragmentNode::KeyBlock(_) => "{#key} block",
        FragmentNode::SnippetBlock(_) => "{#snippet} block",
        FragmentNode::HtmlTag(_) => "{@html} tag",
        FragmentNode::ConstTag(_) => "{@const} tag",
        FragmentNode::DeclarationTag(_) => "declaration tag",
        FragmentNode::DebugTag(_) => "{@debug} tag",
        FragmentNode::RenderTag(_) => "{@render} tag",
    }
}

/// The scoping analysis product: which class names the style scopes, and the
/// host-source positions where the hash class splices into the style text.
struct ScopeInfo {
    class_names: BTreeSet<String>,
    /// Host-source byte offsets (each just past a `.class` selector token)
    /// where `.svelte-tsvhash` is inserted, ascending.
    insertions: Vec<u32>,
}

/// Analyze a `<style>` for the minimal supported shape: top-level rules whose
/// selectors are single simple class selectors. Anything else is refused — the
/// real matcher/pruner machinery is a later milestone.
fn analyze_style(style: &Style<'_>, source: &str) -> Result<ScopeInfo, CompileError> {
    let mut info = ScopeInfo {
        class_names: BTreeSet::new(),
        insertions: Vec::new(),
    };
    for node in style.css_stylesheet.nodes {
        let CssNode::Rule(rule) = node else {
            return Err(unsupported(Refusal::CssAtRule));
        };
        for child in rule.declarations {
            if matches!(child, CssBlockChild::Rule(_) | CssBlockChild::Atrule(_)) {
                return Err(unsupported(Refusal::CssNestedRule));
            }
        }
        for complex in rule.selector.selectors {
            let [relative] = complex.children else {
                return Err(unsupported(Refusal::CssCombinatorSelector));
            };
            let [SimpleSelector::Class { span }] = relative.selectors else {
                return Err(unsupported(Refusal::CssNonClassSelector));
            };
            // Span text includes the leading `.`.
            let name = &span.extract(source)[1..];
            info.class_names.insert(name.to_string());
            info.insertions.push(span.end);
        }
    }
    info.insertions.sort_unstable();
    Ok(info)
}

/// The scoped CSS: the author's style text verbatim (whitespace preserved) with
/// `.svelte-tsvhash` spliced in after each scoped selector — a source splice,
/// not a reprint, matching the oracle's output byte-for-byte.
fn splice_scoped_css(style: &Style<'_>, source: &str, scope: &ScopeInfo) -> String {
    let content_start = style.content_span.start;
    let content = style.content_span.extract(source);
    let mut out = String::with_capacity(content.len() + 16 * scope.insertions.len());
    let mut prev = 0usize;
    for &pos in &scope.insertions {
        let rel = (pos - content_start) as usize;
        out.push_str(&content[prev..rel]);
        out.push('.');
        out.push_str(SCOPE_HASH_CLASS);
        prev = rel;
    }
    out.push_str(&content[prev..]);
    out
}
