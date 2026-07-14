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

use std::collections::{BTreeSet, HashMap};

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::Span;
use tsv_svelte::ast::internal::Root;
use tsv_ts::ast::internal::{
    BlockStatement, ExportDefaultDeclaration, ExportDefaultValue, Expression, ExpressionStatement,
    FunctionDeclaration, Statement, VariableDeclaration, VariableDeclarationKind,
};

use crate::analyze::{Bindings, NameSet, RuneInit, Scope, ScopeEntry, classify_rune_init};
use crate::blocks::declaration_stmt;
use crate::build::Builder;
use crate::css_scope::{ScopeInfo, analyze_style, splice_scoped_css};
use crate::element::fragment_has_component;
use crate::fragment::{
    BodyBuilder, FragmentCtx, emit_fragment, fragment_contains_block,
    fragment_has_snippet_or_render,
};
use crate::script_rewrite::{
    analyze_script, collect_script_comments, document_ts_flag, identifier_binding_name,
    plain_identifier_name, refuse_runes_invalid_import, refuse_template_typescript,
    rewrite_script_statement, self_check_no_typescript,
};
use crate::snippet::SnippetAnalysis;
use crate::{CompileError, CompileOutput, Refusal, erase};

/// The component function name. Derived from the constant filename the
/// deterministic oracle compiles under (`input.svelte` → `Input`).
const COMPONENT_NAME: &str = "Input";

/// Everything the emitters share: the builder, the script analysis products,
/// and the CSS scoping state.
pub(crate) struct EmitEnv<'arena, 's> {
    pub(crate) b: Builder<'arena>,
    pub(crate) source: &'s str,
    pub(crate) bindings: Bindings<'arena>,
    pub(crate) derived_names: NameSet,
    /// Top-level `$state`/`$state.raw` binding names. Stored as `Normal` in the
    /// binding table (a plain server variable after rewrite), but the oracle's
    /// `binding.kind` is `state` (non-`normal`), so a component whose name is a
    /// `$state` binding is dynamic — this set recovers that distinction.
    pub(crate) state_names: NameSet,
    pub(crate) scope: Option<ScopeInfo>,
    pub(crate) matched_classes: BTreeSet<String>,
    /// Script comments are being carried — emitters whose synthetic call
    /// windows would sweep host comments (`$.attr` family) must refuse.
    pub(crate) has_comments: bool,
    /// Active block-scope overlays (each items/indexes, `{:then}` values,
    /// `{@const}` bindings), innermost last.
    pub(crate) overlays: Vec<HashMap<String, ScopeEntry<'arena>>>,
    /// Inside an `{#each}` body — a nested each would need the oracle's
    /// unique-name allocation order, which is not confidently reproducible.
    pub(crate) in_each: bool,
    /// Source-order counters for the oracle's per-each unique names
    /// (`each_array`/`each_array_1`, `$$index`/`$$index_1`), advanced once per
    /// each block regardless of an authored index.
    each_array_count: usize,
    index_count: usize,
    /// The snippet hoist analysis: which top-level snippets go to module scope,
    /// and every snippet name (render-callee classification, name collisions).
    pub(crate) snippets: SnippetAnalysis,
    /// Module-scope `function` declarations for hoistable top-level snippets,
    /// emitted between the imports and the exported component function (source
    /// order — a top-level snippet is visited in order as the root fragment is
    /// walked).
    pub(crate) hoisted_snippets: Vec<Statement<'arena>>,
    /// Comment-refusal windows of the regions erased from **template** borrow
    /// points (the script's are collected before emission). See
    /// [`EmitEnv::erase`].
    erased_windows: Vec<Span>,
}

impl<'arena> EmitEnv<'arena, '_> {
    pub(crate) fn value_scope(&self) -> Scope<'_, 'arena> {
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
    pub(crate) fn erase(
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
    pub(crate) fn push_overlay(
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

    pub(crate) fn pop_overlay(&mut self) {
        self.overlays.pop();
    }

    /// The `each_array` unique name for the next `{#each}` (source order).
    pub(crate) fn next_each_array_name(&mut self) -> String {
        let n = self.each_array_count;
        self.each_array_count += 1;
        if n == 0 {
            "each_array".to_string()
        } else {
            format!("each_array_{n}")
        }
    }

    /// The `$$index` unique name for the next index-less `{#each}`.
    pub(crate) fn next_index_name(&mut self) -> String {
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

pub(crate) fn unsupported(reason: Refusal) -> CompileError {
    CompileError::Unsupported(reason)
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
