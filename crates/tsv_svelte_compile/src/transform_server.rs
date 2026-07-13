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
    ElementKind, ExpressionTag, Fragment, FragmentNode, HtmlTag, IfBlock, KeyBlock, Root,
    SpecialElement, SpecialElementKind, Style,
};
use tsv_ts::ast::internal::{
    BinaryOperator, BlockStatement, ExportDefaultDeclaration, ExportDefaultValue, Expression,
    ExpressionStatement, ForInit, ForStatement, FunctionDeclaration, IfStatement, ObjectPattern,
    ObjectPatternProperty, Property, PropertyKind, RestElement, Statement, UpdateOperator,
    VariableDeclaration, VariableDeclarationKind, VariableDeclarator,
};

use std::collections::HashMap;

use crate::analyze::{
    Binding, BindingKind, Bindings, Initial, NameSet, RuneInit, Scope, ScopeEntry,
    classify_rune_init, evaluate, is_effect_call, pattern_binding_names, stringify_value,
};
use crate::build::{Builder, escape_template_text};
use crate::rune_guard::{WalkCtx, walk_expression_guarded, walk_statement_guarded};
use crate::{CompileError, CompileOutput, Refusal};

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
}

impl<'arena> EmitEnv<'arena, '_> {
    fn value_scope(&self) -> Scope<'_, 'arena> {
        Scope {
            bindings: &self.bindings,
            overlays: &self.overlays,
        }
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
    // TS instance scripts pass type annotations through verbatim (type stripping
    // isn't implemented), and `generics` implies TS — refuse both at the entry,
    // before any divergent output could emit.
    if let Some(script) = root.instance {
        refuse_typed_script(script, source)?;
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
    let script_comments = collect_script_comments(root, source)?;
    let has_comments = !script_comments.is_empty();
    // Comments alongside template blocks refuse: a block splits the template
    // into multiple pushes and moves content into branch bodies, and the
    // resulting comment-window placement is unprobed — refuse rather than risk a
    // misplaced comment.
    if has_comments && fragment_contains_block(&root.fragment) {
        return Err(unsupported(Refusal::CommentsAlongsideTemplateBlocks));
    }

    // 3. Script analysis pass: the top-level binding table (evaluator input)
    // and the derived-name set (read rewriting / refusal).
    let mut bindings = Bindings::empty();
    let mut derived_names = NameSet::default();
    if let Some(script) = root.instance {
        analyze_script(
            script.content.body,
            source,
            &mut bindings,
            &mut derived_names,
        )?;
    }
    if has_comments && !derived_names.is_empty() {
        // The `$.derived(() => …)` wrapper and `d()` reads bridge host and
        // appendix spans in ways whose comment windows sweep wrongly.
        return Err(unsupported(Refusal::CommentsWithDerived));
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
    let component = crate::needs_context::analyze_component(root, source);
    let uses_slots = component.as_ref().is_ok_and(|c| c.uses_slots);
    if let Some(script) = root.instance {
        for stmt in script.content.body {
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
            if matches!(stmt, Statement::ImportDeclaration(_)) {
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
    }

    let mut env = EmitEnv {
        b,
        source,
        bindings,
        derived_names,
        scope,
        matched_classes: BTreeSet::new(),
        has_comments,
        overlays: Vec::new(),
        in_each: false,
        each_array_count: 0,
        index_count: 0,
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
            preserve_whitespace: false,
            parent_name: None,
        },
    )?;
    let body = out.finish(&mut env.b, arena);

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
        let span = call.span();
        let mut outer: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        outer.push(Statement::ExpressionStatement(ExpressionStatement {
            expression: call,
            span,
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

    let mut export_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    export_body.push(Statement::ExportDefaultDeclaration(export));
    let export_program = tsv_ts::ast::internal::Program {
        body: export_body.into_bump_slice(),
        comments: script_comments,
        span: Span::new(0, env.b.buffer.len() as u32),
        interner: std::rc::Rc::clone(&root.interner),
        goal: tsv_ts::Goal::Module,
    };

    let mut js = tsv_ts::format_canonical(&import_program, &env.b.buffer);
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
) -> Result<Vec<tsv_lang::Comment>, CompileError> {
    if root.comments.is_empty() {
        return Ok(Vec::new());
    }
    let Some(script) = root.instance else {
        return Err(unsupported(Refusal::TemplateComments));
    };
    let content = script.content.span;
    // A comment after the LAST script statement diverges: the oracle's printer
    // re-attaches it as a leading comment of the next emitted node (inside the
    // template's `$.escape(…)` argument), a placement this transform can't
    // reproduce — refuse the class.
    let last_stmt_end = script
        .content
        .body
        .last()
        .map_or(content.start, |stmt| stmt.span().end);
    // A leading comment glued to the `<script>` line (no newline before it) shares
    // its source line with the function's synthetic opening brace, so the printer
    // trails it after the `{` instead of onto its own line — refuse the class
    // (prettier-formatted input always puts a leading comment on its own line, so
    // the covered fixtures are unaffected).
    let first_stmt_start = script
        .content
        .body
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
        comments.push(comment.clone());
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

/// Refuse a `<script>` that carries a `lang` (TypeScript) or `generics`
/// attribute: the transform emits the script body verbatim, so a TS annotation
/// would leak into the generated JS. Mirrors the attribute walk in the parser's
/// `detect_script_context` — only plain `Attribute` nodes carry a name here.
fn refuse_typed_script(
    script: &tsv_svelte::ast::internal::Script<'_>,
    source: &str,
) -> Result<(), CompileError> {
    for attr_node in script.attributes {
        let AttributeNode::Attribute(attr) = attr_node else {
            continue;
        };
        let name = {
            let interner = script.content.interner.borrow();
            interner.resolve_infallible(attr.name).to_string()
        };
        match name.as_str() {
            "lang" => {
                // The oracle treats `lang="js"` and `lang=""` as plain JS
                // (probe-verified identical output to no attribute) — allow
                // exactly those two; every other value (or a bare /
                // expression-valued `lang`) stays refused.
                let allowed = match attr.value {
                    Some([AttributeValue::Text(text)]) => {
                        let v = text.data(source);
                        v.is_empty() || v == "js"
                    }
                    Some([]) => true,
                    _ => false,
                };
                if !allowed {
                    let lang = match attr.value {
                        Some([AttributeValue::Text(text)]) => text.data(source).into_owned(),
                        _ => String::new(),
                    };
                    return Err(unsupported(Refusal::LangInstanceScript { lang }));
                }
            }
            "generics" => {
                return Err(unsupported(Refusal::GenericsAttribute));
            }
            _ => {}
        }
    }
    Ok(())
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
                let arrow = b.arrow_expr(expr);
                let arrow_alloc = b.arena.alloc(arrow);
                Some(b.member_call("$", "derived", std::slice::from_ref(arrow_alloc)))
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
    /// Cross-chunk `${` seam invariant: each chunk is template-escaped
    /// independently, so a literal `$` ending one chunk followed by a literal
    /// `{` starting the next would slip through unescaped. That pairing is
    /// unreachable — a decoded text run is always a single chunk (the parser
    /// yields one `Text` node per run, entities included), and every other
    /// chunk this transform appends starts with `<`, `/`, `>`, a space, or an
    /// anchor comment (`<!…`) — but assert it so a future emitter change fails
    /// loudly.
    fn push_text(&mut self, chunk: &str) {
        // Every element of `texts` exists by construction (starts with one entry;
        // `push_expr` appends the follower).
        #[allow(clippy::unwrap_used)]
        let current = self.texts.last_mut().unwrap();
        debug_assert!(
            !(current.ends_with('$') && chunk.starts_with('{')),
            "cross-chunk `${{` would defeat template escaping"
        );
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
struct FragmentCtx<'p> {
    /// Whether a text-first fragment gets the leading `<!---->` anchor. True for
    /// the component root and `{#each}` bodies (the oracle's `is_text_first`:
    /// parent ∈ {Fragment, SnippetBlock, EachBlock, Component, …}); false for
    /// element children and `{#if}`/`{#key}`/`{#await}` bodies.
    mark_text_first: bool,
    /// Whether this is the component's root fragment (a `{@const}` here refuses —
    /// grammatically block-only, and its component-scope placement is unprobed).
    is_component_root: bool,
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

    // Emit hoisted `{@const}` declarations first — they precede the anchor's
    // following content and enter the evaluator's innermost overlay so later
    // reads in this fragment fold. `<svelte:head>` is hoisted the same way; a
    // fragment carrying both can't fix their relative order (the oracle keeps
    // source order across all hoisted kinds), so refuse that combination.
    if !head_nodes.is_empty() && !const_tags.is_empty() {
        return Err(unsupported(Refusal::SvelteHeadWithConstTag));
    }
    for tag in &const_tags {
        emit_const_tag(env, tag, out)?;
    }
    for head in &head_nodes {
        emit_svelte_head(env, head, out)?;
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

    for node in &list {
        match node {
            CleanNode::Text(data) => {
                out.push_text(&escape_template_text(&escape_html_text(data)));
            }
            CleanNode::Element(element) => {
                emit_element(env, element, out, &ctx)?;
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

/// Guard-walk a dropped expression (a `{#key}` / `{#each (key)}` expression the
/// SSR output ignores) so stray runes / derived reads inside still refuse.
fn guard_dropped<'arena>(
    env: &EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
) -> Result<(), CompileError> {
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let mut ctx = WalkCtx::new(env.source, &mut updated, &mut nested, &env.derived_names);
    walk_expression_guarded(expr, &mut ctx)
}

/// Refuse if a generated block name (`each_array`, `$$index`, `$$length`) would
/// collide with a user binding — the oracle's component-scope name generation
/// would then pick a different suffix, which this port doesn't replicate.
fn check_name_free(env: &EmitEnv<'_, '_>, name: &str) -> Result<(), CompileError> {
    if env.bindings.contains(name) {
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
    // Only a plain-identifier binding is modeled: a destructured `{@const}`
    // whose init folds would have the oracle fold each read, which this port
    // can't reproduce per-binding — refuse rather than risk a silent mismatch.
    let Expression::Identifier(id) = &tag.id else {
        return Err(unsupported(Refusal::DestructuredConstTag));
    };
    let Some(name) = plain_identifier_name(id, env.source) else {
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
    let init = wrap_single(env, &tag.init)?;
    let id_expr = tag.id.clone();
    let arena = env.b.arena;
    let stmt = declaration_stmt(&env.b, VariableDeclarationKind::Const, id_expr, init);
    out.push_statement(&mut env.b, arena, stmt);

    // Enter the innermost overlay so `{name}` reads fold through its init.
    let binding = Binding {
        kind: BindingKind::Normal,
        initial: Initial::Expr(&tag.init),
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
    // in either); the key is then dropped — SSR ignores it.
    if let Some(key) = &each.key {
        guard_dropped(env, key)?;
    }
    if let Some(context) = &each.context {
        guard_dropped(env, context)?;
    }

    // Collection (guard + bare-derived rewrite).
    let collection = wrap_single(env, &each.expression)?;

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
    if let Some(context) = &each.context {
        let mut names = Vec::new();
        pattern_binding_names(context, env.source, &mut names)?;
        for name in names {
            overlay.insert(name, ScopeEntry::Masked);
        }
    }
    overlay.insert(index_name.clone(), ScopeEntry::Masked);
    env.in_each = true;
    let body_result = emit_each_body(env, each, &array_name, &index_name, preserve, overlay);
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
/// the body fragment (which gets the text-first `<!---->` marker).
fn emit_each_body<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    each: &'arena EachBlock<'arena>,
    array_name: &str,
    index_name: &str,
    preserve: bool,
    overlay: HashMap<String, ScopeEntry<'arena>>,
) -> Result<&'arena [Statement<'arena>], CompileError> {
    let arena = env.b.arena;
    let mut pre: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    if let Some(context) = &each.context {
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

    let expr = wrap_single(env, &await_block.expression)?;

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
    let then_params: &'arena [Expression<'arena>] = match &await_block.value {
        Some(value) => {
            guard_dropped(env, value)?;
            std::slice::from_ref(arena.alloc(value.clone()))
        }
        None => &[],
    };
    let then_stmts = match &await_block.then {
        Some(frag) => {
            let mut overlay: HashMap<String, ScopeEntry<'arena>> = HashMap::new();
            if let Some(value) = &await_block.value {
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

/// Emit `{expr}` (escaped) or `{@html expr}` (raw) — the oracle's text-sequence
/// interpolation with its fold gate.
fn emit_expression_tag<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    expr: &'arena Expression<'arena>,
    out: &mut BodyBuilder<'arena>,
    escape: bool,
) -> Result<(), CompileError> {
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
) -> Result<(), CompileError> {
    let name = env
        .b
        .interner
        .borrow()
        .resolve_infallible(element.name)
        .to_string();
    // The oracle compiles a component (`<Foo>`, `<Foo.Bar>`) into a call
    // (`Foo($$renderer, {})`), never static markup — component rendering is a
    // later milestone. Exhaustive over `ElementKind` so a new kind can't silently
    // fall through to the static-HTML path below.
    match element.kind {
        ElementKind::Html => {}
        ElementKind::Component => {
            return Err(unsupported(Refusal::ComponentElement {
                name: name.clone(),
            }));
        }
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
            preserve_whitespace: parent_ctx.preserve_whitespace
                || name == "pre"
                || name == "textarea",
            parent_name: Some(&name),
        },
    )?;
    out.push_text(&format!("</{name}>"));
    Ok(())
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
                return Ok(());
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
    // A string-literal expression value takes the oracle's inline-literal path
    // (pre-escaped static emission) — refuse rather than guess its edge rules.
    if matches!(expr, Expression::Literal(lit)
        if matches!(lit.value, tsv_ts::ast::internal::LiteralValue::String(_)))
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
                // Guard first — never fold an oracle-invalid expression.
                let wrapped = wrap_value_expr(env, &tag.expression)?;
                let evaluated = evaluate(&tag.expression, &env.value_scope(), env.source, 0)
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
