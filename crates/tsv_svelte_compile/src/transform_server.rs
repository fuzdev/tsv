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

use std::collections::HashMap;

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{Comment, Span};
use tsv_svelte::ast::internal::{Element, Root, SpecialElement};
use tsv_ts::ast::internal::{
    BlockStatement, ExportDefaultDeclaration, ExportDefaultValue, Expression, ExpressionStatement,
    FunctionDeclaration, ObjectExpression, ObjectProperty, Statement, VariableDeclaration,
    VariableDeclarationKind,
};

use crate::analyze::{Bindings, NameSet, RuneInit, Scope, ScopeEntry, classify_rune_init};
use crate::blocks::{self, declaration_stmt};
use crate::body_builder::BodyBuilder;
use crate::build::{Builder, init_property};
use crate::css_scope::{CssScoping, analyze_style, match_scope, splice_scoped_css};
use crate::element_census::build_census;
use crate::fragment::{FragmentCtx, emit_fragment};
use crate::namespace::{FragmentParent, Namespace, infer_namespace};
use crate::needs_context::{ComponentContext, analyze_component, collect_constant_names};
use crate::script_bindings::{analyze_module_script, analyze_script, refuse_runes_invalid_import};
use crate::script_collision::refuse_rune_store_collision;
use crate::script_comments::collect_script_comments;
use crate::script_decls::{identifier_binding_name, plain_identifier_name};
use crate::script_props::BindableEntry;
use crate::script_rewrite::rewrite_script_statement;
use crate::script_ts_gate::{
    document_ts_flag, refuse_template_typescript, self_check_no_typescript,
};
use crate::snippet::{SnippetAnalysis, analyze_snippets};
use crate::{CompileError, CompileOutput, Refusal, erase, store_rewrite};

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
    /// Top-level names a `bind:` may NOT write to (`const` declarators + import
    /// locals, from the instance **and** module scripts) — the oracle's
    /// `constant_binding`. See `attribute::reassignable_bind_target_root`.
    pub(crate) unassignable_names: NameSet,
    /// Top-level binding names, for recognizing a `$name` store auto-subscription
    /// (`$name` where the `$`-stripped base is a binding → `$.store_get`). Read by
    /// the template value walk ([`template_value::wrap_value_expr`](crate::template_value::wrap_value_expr)).
    pub(crate) store_names: NameSet,
    /// Store bases bound in a nested scope (`nested_declared` ∪
    /// `component.fn_declared`). A `$name` store read whose base is here is the
    /// oracle's `store_invalid_scoped_subscription`; the dropped-region guard
    /// refuses it. See [`dropped::guard_dropped`](crate::dropped::guard_dropped).
    pub(crate) store_shadowed: NameSet,
    /// Whether the component makes any valid `$name` store reference anywhere
    /// (read or write, emitted or dropped) — forces the `var $$store_subs;` /
    /// `if ($$store_subs) $.unsubscribe_stores($$store_subs);` component-body
    /// injection. Computed upfront by [`needs_context::analyze_component`](crate::needs_context::analyze_component), the
    /// oracle's analysis-driven store-subscription gate (so a store referenced
    /// only in a dropped handler still injects), NOT set at emission time.
    pub(crate) uses_stores: bool,
    /// The finished CSS scoping — selector→element matching ran upfront over the
    /// [element census](crate::element_census) in [`analyze`], so emission only
    /// *reads* it (`element_scope` is a span lookup, `unused_selectors` the
    /// post-emission no-match check, and `splice_scoped_css` consults the per-relative
    /// scoped flags). `None` when the component has no `<style>`.
    pub(crate) scope: Option<CssScoping>,
    /// Active block-scope overlays (each items/indexes, `{:then}` values,
    /// `{@const}` bindings), innermost last.
    pub(crate) overlays: Vec<HashMap<String, ScopeEntry<'arena>>>,
    /// Inside an `{#each}` body — a nested each refuses. Not because the oracle's
    /// unique-name allocation order is unreachable (it is modelled; see
    /// [`Refusal::NestedEach`]) but because the rest of the nested emission path
    /// carries no coverage.
    pub(crate) in_each: bool,
    /// The one element position an `animate:` directive is legal: the span of the
    /// sole non-trivial child of the enclosing keyed `{#each}` body (decided in
    /// `blocks::animate_host_element`). `element::emit_element` matches an
    /// element's span against this to accept exactly that placement; every other
    /// `animate:` refuses (the oracle's phase-2 placement check).
    pub(crate) animate_host_span: Option<Span>,
    /// Source-order counter for the oracle's per-each `each_array` name
    /// (`each_array`/`each_array_1`), advanced once per emitted each block. The
    /// oracle mints it in the transform, so emission order IS its order.
    each_array_count: usize,
    /// The `$$index` name of every `{#each}` in the component, keyed by block
    /// span. Assigned upfront by [`blocks::assign_each_index_names`] because the
    /// oracle mints it in the **scope** pass — post-order, and over regions the
    /// SSR transform drops — so emission order is the wrong order for it.
    each_index_names: HashMap<(u32, u32), String>,
    /// The snippet hoist analysis: which top-level snippets go to module scope,
    /// and every snippet name (render-callee classification, name collisions).
    pub(crate) snippets: SnippetAnalysis,
    /// Module-scope `function` declarations for hoistable top-level snippets,
    /// emitted between the imports and the exported component function (source
    /// order — a top-level snippet is visited in order as the root fragment is
    /// walked).
    pub(crate) hoisted_snippets: Vec<Statement<'arena>>,
    /// Comment-refusal windows of the regions erased from **template** borrow
    /// points (the script's are an [`Analysis`] product, collected before
    /// emission). Deliberately **not** an [`Analysis`] product: it is accumulated
    /// lazily by [`EmitEnv::erase`] as each template expression is reached, so it
    /// stays `EmitEnv`-only (precomputing it upfront is a deferred option). See
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

    /// Whether this regular element is CSS-scoped: did the upfront census match
    /// give it the `svelte-tsvhash` class? A pure lookup — the matching (including
    /// which selectors are used) already ran in [`analyze`]. Returns `false` when
    /// the component has no `<style>`.
    pub(crate) fn element_scope(&self, element: &Element<'arena>) -> bool {
        self.scope
            .as_ref()
            .is_some_and(|scope| scope.element_scoped(element))
    }

    /// Whether this `<svelte:element>` is CSS-scoped — the same upfront census-match
    /// lookup as [`element_scope`](Self::element_scope), keyed on the special
    /// element's span. A type or universal selector matches a `<svelte:element>`
    /// unconditionally, so a styled component scopes every one it reaches.
    pub(crate) fn special_element_scope(&self, special: &SpecialElement<'arena>) -> bool {
        self.scope
            .as_ref()
            .is_some_and(|scope| scope.special_element_scoped(special))
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

    /// The `$$index` name assigned upfront to the `{#each}` block at `span`.
    ///
    /// The upfront walk rides the exhaustive `each_child_fragment` seam, so every
    /// `{#each}` the parse produced is in the table and a miss cannot happen for a
    /// block reached through emission. A miss would mean the walk lost a fragment
    /// kind — a compiler bug, surfaced as [`CompileError::GeneratedNameMissing`]
    /// rather than guessed. Falling back to the unsuffixed `$$index` would be
    /// *silently correct* in every single-`{#each}` document and a MISMATCH only
    /// in a multi-each one, so the guess would hide the table-population bug in
    /// exactly the documents most likely to be probed first.
    ///
    /// # Errors
    ///
    /// [`CompileError::GeneratedNameMissing`] when `span` is not in the table.
    pub(crate) fn each_index_name(&self, span: Span) -> Result<String, CompileError> {
        let key = (span.start, span.end);
        debug_assert!(
            self.each_index_names.contains_key(&key),
            "every {{#each}} must have been assigned a $$index name upfront"
        );
        self.each_index_names
            .get(&key)
            .cloned()
            .ok_or(CompileError::GeneratedNameMissing(span))
    }
}

/// The order-independent analysis products of a component, plus the raw inputs
/// the script-rewrite loop still consumes.
///
/// Produced by [`analyze`], which runs the whole SSR setup **except** the two
/// transform-local parts (`EmitEnv` construction and the script-rewrite loop).
/// Two products are deliberately *not finished* here because their finalization
/// depends on the loop's output — see the [`Analysis::bindings`] and
/// [`Analysis::component`] field notes.
pub(crate) struct Analysis<'arena> {
    /// The top-level script binding table (the evaluator's input).
    ///
    /// ⚠ **Not frozen when `analyze` returns.** It is populated by
    /// [`analyze_script`] but the `mark_updated` / `mark_opaque` patch is
    /// deferred to [`compile_server`]: that patch reads `updated` /
    /// `nested_declared` from the rewrite loop and `reassigned` / `fn_declared`
    /// from [`Self::component`], neither of which exists until the loop runs.
    /// [`compile_server`] finalizes it in place, the same mutation sequence as
    /// before this factoring.
    pub(crate) bindings: Bindings<'arena>,
    /// Derived-binding names (read rewriting / refusal).
    pub(crate) derived_names: NameSet,
    /// Top-level `$state`/`$state.raw` binding names — a component named after
    /// one is dynamic (see `component_dynamic`).
    pub(crate) state_names: NameSet,
    /// Top-level names a `bind:` may NOT write to: every `const` declarator name and
    /// every import local, from the instance **and** module scripts — the rule reads
    /// the declaration KEYWORD, so which script declared the name is immaterial.
    /// Deliberately SEPARATE from `state_names` rather than
    /// subtracted from it — `state_names` also drives the component-dynamic
    /// classification, where a `const c = $state(…)` is still reactive and still
    /// dynamic. See `attribute::reassignable_bind_target_root`.
    pub(crate) unassignable_names: NameSet,
    /// The finished `<style>` scoping — selectors parsed **and** matched against the
    /// element census upfront (scoped-element set, per-selector used flags,
    /// per-relative hash-splice flags). `None` when the component has no `<style>`.
    pub(crate) scope: Option<CssScoping>,
    /// Whether the instance script carries comments.
    pub(crate) has_comments: bool,
    /// The `{#snippet}` hoist analysis.
    pub(crate) snippets: SnippetAnalysis,
    /// The whole-component `needs_context`/reassignment analysis, **left
    /// unresolved on purpose.** [`analyze_component`] is called up front (the
    /// script rewrite needs its `uses_slots`), but its error must NOT win over
    /// the script-rewrite loop's refusals — the oracle's refusal priority puts
    /// the loop first. So [`compile_server`] reads `uses_slots` from this
    /// `Result` before the loop and only `?`-propagates it after. An eager `?`
    /// here would flip refusal priority on any file both would decline.
    pub(crate) component: Result<ComponentContext, CompileError>,
    /// The type-erased instance-script statement list — the rewrite loop
    /// iterates it. (NOTHING may read `root.instance.content.body`: the un-erased
    /// tree still carries TypeScript.)
    pub(crate) instance_body: &'arena [Statement<'arena>],
    /// The type-erased module-script statement list (imports + declarations +
    /// non-default exports, source order), emitted **verbatim** as a comment-free
    /// module-scope program between the hoisted snippets and the component
    /// function. Empty when there is no module script. (NOTHING may read
    /// `root.module.content.body`: the un-erased tree still carries TypeScript.)
    pub(crate) module_body: &'arena [Statement<'arena>],
    /// Carried instance-script comments (threaded into the export program).
    pub(crate) script_comments: Vec<Comment>,
    /// Comment-refusal windows of the regions erased from the **script**. (The
    /// template half is *not* here: it is accumulated lazily at each
    /// [`EmitEnv::erase`] borrow point and stays `EmitEnv`-only — see that
    /// field. Precomputing it is a deferred option, not this slice's.)
    pub(crate) erased_windows: Vec<Span>,
    /// The `$$index` generated name of every `{#each}` in the component, keyed by
    /// block span — the oracle's scope-pass, post-order allocation. See
    /// [`blocks::assign_each_index_names`].
    pub(crate) each_index_names: HashMap<(u32, u32), String>,
}

/// Run the component's order-independent analysis passes.
///
/// # Boundary
///
/// This is the SSR setup block **minus** the two transform-local pieces:
/// `EmitEnv` construction and the script-rewrite loop. The loop mints appendix
/// lexemes through the [`Builder`] and sits *between* the products here, and two
/// of those products can only be *finished* after it runs. Rather than thread the
/// builder through `analyze`, the loop stays inline in [`compile_server`] and
/// `analyze` returns:
///
/// - the products that don't depend on the loop (TypeScript erasure, the CSS
///   scope, carried comments, the binding table, derived/state names, the snippet
///   hoist analysis), and
/// - the two deferred-finalization products as-is: [`Analysis::bindings`]
///   pre-patch and [`Analysis::component`] unresolved (see their field notes).
///
/// `analyze` performs **no minting**, so it needs no [`Builder`]: the setup block
/// mints nothing until the loop, so the `$` runtime import stays the first
/// appendix lexeme whether it is minted before or after this call, and the
/// appendix byte stream is identical either way.
fn analyze<'arena>(
    root: &Root<'arena>,
    source: &str,
    arena: &'arena bumpalo::Bump,
) -> Result<Analysis<'arena>, CompileError> {
    if root.options.is_some() {
        return Err(unsupported(Refusal::SvelteOptions));
    }
    // The emission-independent validation rules (the oracle's parse-time and
    // whole-component checks) — one walk over the whole document, dropped regions
    // included, before any emission decision is made. See `validate.rs`.
    crate::validate::validate_document(root, source)?;
    // TypeScript erasure — the oracle's phase-1 `remove_typescript_nodes`, run
    // BEFORE every analysis pass (`analyze_script`, `analyze_snippets`,
    // `needs_context`) and before the codegen loop. `instance_body` is the
    // type-free statement list from here on; NOTHING below may read
    // `root.instance.content.body` (the un-erased tree still carries TS). The
    // document `ts` flag is decided from the first lang-bearing script in source
    // order — the module can set it (`document_ts_flag`).
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
    // The module `<script>` half: erase + validate (plain modules only — runes,
    // store reads, top-level await, and `export default` refuse). Its type-free
    // body emits verbatim at module scope. Runs before every analysis so the
    // module bindings feed the same passes the instance bindings do.
    let module_body = analyze_module_script(root, source, arena, ts_document)?;
    // The template half of the same document-wide gate. The borrow points
    // (`EmitEnv::erase`) already erase every template expression that reaches
    // output, so this sweep exists for the ones that DON'T — see
    // `refuse_template_typescript`. It runs only without the flag.
    if !ts_document {
        refuse_template_typescript(root, source, arena)?;
    }

    // CSS scoping (no minting): parse the selector chains, then match them against
    // the element census upfront. The census (`element_census`) gives the ancestor/
    // sibling navigability tsv's AST lacks, so the whole selector→element table —
    // which elements gain the hash class, which selectors are used, which compounds
    // splice a hash — is computed here rather than accumulated during emission. A
    // dynamic-attribute / non-ASCII / snippet-crossing match refuses here.
    let scope = match root.css {
        Some(style) => {
            let info = analyze_style(style, source, None)?;
            let census = build_census(root);
            Some(match_scope(info, &census, source)?)
        }
        None => None,
    };

    // Comment carry-through: script comments thread into the synthetic program
    // (their spans are host-absolute, so the detached-comment machinery works
    // against the buffer). Classes whose placement can't be made to converge
    // refuse — see `collect_script_comments`.
    let script_comments = collect_script_comments(root, source, instance_body)?;
    let has_comments = !script_comments.is_empty();

    // A rune keyword whose stem is also a binding in scope at the instance script
    // (`import { state } from './store'` + `$state`) is READ AS A STORE by the
    // oracle, not as the rune — refuse before the binding table (which would
    // classify it as the rune) is built. `instance.scope.get` chains to the module
    // scope, so both bodies are searched. See `refuse_rune_store_collision`.
    refuse_rune_store_collision(instance_body, module_body, source, &root.interner)?;

    // Script analysis pass: the top-level binding table (evaluator input) and
    // the derived-name set (read rewriting / refusal). The instance script fills
    // the table first.
    let mut bindings = Bindings::empty();
    let mut derived_names = NameSet::default();
    analyze_script(instance_body, source, &mut bindings, &mut derived_names)?;

    // The snippet-hoist BLOCKER set is INSTANCE bindings only. A module binding
    // is module-scope (accessible from a hoisted, module-scope snippet, like an
    // import), so it must NOT block hoisting — freeze the set here, before the
    // module bindings join the table.
    let instance_binding_names: NameSet = bindings.names().map(str::to_string).collect();

    // A module↔instance top-level binding-name COLLISION is a real MISMATCH: the
    // shared table below overwrites (module analyzed second), so a template
    // `{name}` read would fold the module value where the oracle resolves the
    // instance (inner-scope) binding. The name-based port can't tell which scope a
    // reference resolves to (a hoisted module-scope snippet may legitimately
    // reference the module binding), so a plain analyze-order swap can't fix it —
    // refuse the collision (zero corpus yield; the corpus has none). Detected on a
    // throwaway table so the check runs before the shared table is mutated.
    if !module_body.is_empty() {
        let mut module_bindings = Bindings::empty();
        let mut module_derived = NameSet::default();
        analyze_script(
            module_body,
            source,
            &mut module_bindings,
            &mut module_derived,
        )?;
        for name in module_bindings.names() {
            if instance_binding_names.contains(name) {
                return Err(unsupported(Refusal::ModuleInstanceNameCollision {
                    name: name.to_string(),
                }));
            }
        }
    }

    // Module bindings join the shared table: they feed the evaluator (a module
    // `const K = 5` folds `{K}`), the store-base set (a template `$c` on a module
    // store), and `needs_context` (a module import member/call).
    analyze_script(module_body, source, &mut bindings, &mut derived_names)?;

    // Snippet hoist analysis: which top-level `{#snippet}`s go to module scope.
    // Import locals don't disqualify hoisting — instance AND module imports — so
    // the blocker set the analysis subtracts is the instance-binding table minus
    // every import local.
    let import_names: NameSet = instance_body
        .iter()
        .chain(module_body.iter())
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
    let snippets = analyze_snippets(root, source, &instance_binding_names, &import_names)?;

    // Every top-level binding name — instance AND module — is a candidate store
    // base: a `$name` reference is a store auto-subscription iff `name` is a
    // binding. The binding NAME set is frozen here (the later
    // `mark_updated`/`mark_opaque` patch changes kinds, not names), so it serves
    // `analyze_component`'s store-injection gate (the same set is recomputed in
    // `compile_server` for the script guard / store rewrite / template walk).
    let store_names: NameSet = bindings.names().map(str::to_string).collect();

    let unassignable_names = collect_constant_names(instance_body, module_body, source);

    // The whole-component analysis, over instance + module + template. Computed
    // here because the script rewrite needs its `uses_slots` (the `$props()` rest
    // injection renames its destructured `$$slots` to `$$slots_` when the injected
    // sanitize_slots binding exists — a duplicate `$$slots` lexical declaration
    // would be invalid JS). Its error is deliberately left UNRESOLVED (see
    // `Analysis::component`): the script-loop refusals must keep winning for
    // inputs that trip both.
    let component = analyze_component(
        root,
        source,
        instance_body,
        module_body,
        &store_names,
        &unassignable_names,
    );

    // Top-level `$state`/`$state.raw` binding names — a component named after one
    // is dynamic (see `component_dynamic`).
    //
    // ⚠️ Scanning `instance_body` alone is safe only BECAUSE a module-script rune
    // refuses upstream: `analyze_module_script` runs its guard walk before this
    // point, so no module `$state` can reach here. It is NOT a rule about module
    // scope — the oracle keys component-dynamism on `binding.kind !== 'normal'`
    // (`2-analyze/visitors/Component.js:14`), and a module `$state` gets kind
    // `state` exactly like an instance one, so `<script module>let Thing =
    // $state(null)</script><Thing />` compiles to the DYNAMIC form. Whoever lifts
    // the module-rune refusal must widen this scan to `module_body` too, or the
    // component emits a static call where the oracle guards it — a MISMATCH.
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

    Ok(Analysis {
        bindings,
        derived_names,
        state_names,
        unassignable_names,
        scope,
        has_comments,
        snippets,
        component,
        instance_body,
        module_body,
        script_comments,
        erased_windows,
        each_index_names: blocks::assign_each_index_names(&root.fragment),
    })
}

/// Compile a parsed component to server output.
pub(crate) fn compile_server<'arena>(
    root: &Root<'arena>,
    source: &str,
    arena: &'arena bumpalo::Bump,
) -> Result<CompileOutput, CompileError> {
    // Analysis first (setup block minus the rewrite loop and `EmitEnv`). See
    // `analyze`: `bindings` comes back pre-patch and `component` unresolved.
    let Analysis {
        mut bindings,
        derived_names,
        state_names,
        unassignable_names,
        scope,
        has_comments,
        snippets,
        component,
        instance_body,
        module_body,
        script_comments,
        erased_windows,
        each_index_names,
    } = analyze(root, source, arena)?;

    // Every top-level binding name is a candidate store base: `$name` is a store
    // auto-subscription iff `name` is a binding (an unbound `$name` is the
    // oracle's `global_reference_invalid` error). The binding NAME set is stable
    // under the later `mark_updated`/`mark_opaque` patch (which changes kinds,
    // not names), so it is frozen here — read by the script guard (below), the
    // store rewrite, and the template value walk (`EmitEnv::store_names`).
    let store_names: NameSet = bindings.names().map(str::to_string).collect();

    // 1. `import * as $ from 'svelte/internal/server';` — the first appendix
    // lexeme. `analyze` mints nothing, so this heads the appendix exactly as it
    // did before the setup block was factored out.
    let mut b = Builder::new(arena, source, std::rc::Rc::clone(&root.interner));
    let import = b.import_namespace("$", "svelte/internal/server");

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
    // Bindable props collected from the `$props()` destructure defaults (source
    // order). A non-empty set forces the `$$renderer.component(…)` wrapper and
    // appends the trailing `$.bind_props($$props, { … })` (below).
    let mut bindable: Vec<BindableEntry> = Vec::new();
    // The `$props.id()` local, if the script declares one (`const id =
    // $props.id()`). The declarator is skipped in the loop and hoisted to the top
    // of the component body below. At most one (`props_duplicate`).
    let mut props_id: Option<String> = None;
    // `component` is the whole-component analysis, computed up front by `analyze`
    // (the script rewrite needs its `uses_slots`) but left UNRESOLVED: its error
    // must NOT win over the script-loop refusals below, so it is `?`-unwrapped
    // only after the loop (see `Analysis::component`).
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
            &store_names,
            &mut updated,
            &mut nested_declared,
            &mut uses_props,
            &mut has_effects,
            has_comments,
            uses_slots,
            &mut dropped_regions,
            &mut bindable,
            &mut props_id,
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
    // The hoisted `const <name> = $.props_id($$renderer)` is a synthetic
    // (appendix-span) first statement, so its leading comment window would sweep
    // every carried script comment — the same hazard the `$$slots` sanitize decl
    // has. Refuse the combination (a safe over-refusal).
    if props_id.is_some() && has_comments {
        return Err(unsupported(Refusal::CommentsWithPropsId));
    }
    // The oracle wraps the whole body in `$$renderer.component(($$renderer) => …)`
    // whenever its `needs_context` analysis fires (a `new` expression or a
    // member/call rooted in a prop/import — see `needs_context`). A dropped
    // `$effect` is one such trigger (already modeled by `has_effects`); the port
    // covers the rest. `needs_context` also forces the `$$props` parameter (the
    // oracle's `should_inject_props` includes `should_inject_context`). The same
    // walk collects component-wide reassignments — including mutations inside
    // dropped event handlers — so a mutated binding is not statically folded.
    // (Computed by `analyze` for `uses_slots`; its error surfaces here — after
    // the loop, so the loop's refusals keep priority.)
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

    // A bindable prop forces the wrapper: the oracle's `$bindable` visitor sets
    // `needs_context` (the wrapper hosts the trailing `$.bind_props(…)`), so any
    // bindable prop is a wrapper trigger just like a dropped `$effect`.
    let needs_context = has_effects || component.needs_context || !bindable.is_empty();
    if needs_context {
        uses_props = true;
    }
    // A `$$slots` reference makes the component inject
    // `const $$slots = $.sanitize_slots($$props)` (below) and take `$$props`
    // (the oracle's `should_inject_props` includes `uses_slots`). Carried script
    // comments plus the injected first statement would sweep the function-body
    // comment windows, so refuse that combination for now.
    // A store reference makes the component inject `var $$store_subs;` as a
    // component-body statement (below). Being a synthetic (appendix-span)
    // statement, it sweeps the function-body / wrapper-block leading comment
    // window exactly as the `$$slots` injection does, so refuse a carried script
    // comment alongside it — including a template-only `$name` read, which still
    // injects the var. (The script-position store rewrite mints — `$.store_get`
    // / `$.store_set` — sweep the same way; both are covered here since either
    // implies `uses_stores`.)
    if component.uses_stores && has_comments {
        return Err(unsupported(Refusal::CommentsWithStore));
    }
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

    // A store base bound in a nested scope is the oracle's
    // `store_invalid_scoped_subscription` — the store rewrite (and the dropped
    // guard) refuse a store read whose base is here. `nested_declared` (script
    // side) ∪ `component.fn_declared` (whole component, template included) is the
    // name-based shadow set: a conservative superset (a sibling-scope collision
    // over-refuses, which is safe — never an over-acceptance).
    let mut store_shadowed = nested_declared.clone();
    store_shadowed.extend(component.fn_declared.iter().cloned());

    // A `$derived` name shadowed by a NESTED-scope binding of the emitted script
    // (a parameter or nested local) can't be rewritten safely: the read rewrite
    // below is name-based, so a read of the shadowing binding would become `d()`
    // where the oracle keeps the bare binding — a MISMATCH. Shadowing a derived is
    // LEGAL (unlike a store, which the oracle rejects), so refusing is a tsv-side
    // over-refusal; keep it as narrow as the risk by checking `nested_declared`
    // (the emitted-script nested scopes this rewrite descends), NOT the wider
    // `store_shadowed` (which also folds in template-handler locals the store/
    // derived rewrite never touches). A safe over-refusal, rare.
    if let Some(name) = derived_names
        .iter()
        .find(|name| nested_declared.contains(*name))
    {
        return Err(unsupported(Refusal::DerivedReadShadowed {
            name: name.clone(),
        }));
    }

    // Rewrite script-position store reads and writes over the FINAL synthetic body
    // (after erasure + rune rewrites), so a read inside a `$.derived(() => …)`
    // thunk is reached too: `$name` → `$.store_get(…)`, `$name = v` →
    // `$.store_set(…)`, `$name++` → `$.update_store(…)`; a member/destructuring
    // write, or a read whose base is shadowed, refuses. The `var $$store_subs`
    // injection is analysis-driven (`component.uses_stores`) and independent of
    // this rewrite — it fires for a store referenced only in a dropped handler too.
    let body_slice = body.into_bump_slice();
    let body = store_rewrite::rewrite_store_accesses(
        &mut b,
        source,
        &store_names,
        &store_shadowed,
        &derived_names,
        body_slice,
    )?
    .unwrap_or(body_slice);

    let mut env = EmitEnv {
        b,
        source,
        bindings,
        derived_names,
        state_names,
        unassignable_names,
        store_names,
        store_shadowed,
        uses_stores: component.uses_stores,
        scope,
        overlays: Vec::new(),
        in_each: false,
        animate_host_span: None,
        each_array_count: 0,
        each_index_names,
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
        out.stmts.push(stmt.clone());
    }
    // The root fragment re-infers its namespace from its own nodes (Svelte's
    // `Fragment` visitor with the `Root` parent — in the deep-walk special list),
    // starting from the html document default.
    let root_namespace = infer_namespace(
        &env,
        Namespace::Html,
        FragmentParent::Special,
        root.fragment.nodes,
    );
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
            namespace: root_namespace,
            in_svg_text: false,
        },
    )?;
    // `if ($$store_subs) $.unsubscribe_stores($$store_subs);` is the store-cleanup
    // statement — the component body's last statement, but BEFORE any
    // `$.bind_props` (the oracle's order).
    if env.uses_stores {
        let stmt = env.b.unsubscribe_stores_stmt();
        out.push_statement(&mut env.b, arena, stmt);
    }
    // `$.bind_props($$props, { … })` is the component body's LAST statement (after
    // every template flush, inside the `needs_context` wrapper forced above),
    // listing the bindable props in source order — shorthand when the prop key and
    // the local name match, `key: local` when renamed.
    if !bindable.is_empty() {
        let stmt = build_bind_props_stmt(&mut env.b, arena, &bindable);
        out.push_statement(&mut env.b, arena, stmt);
    }
    let body = out.finish(&mut env.b, arena);

    // `var $$store_subs;` is prepended when any store read compiled — BEFORE the
    // `$props.id()` hoist prepend below, so the final order is
    // `const id = $.props_id(…)` then `var $$store_subs;` (the oracle's order).
    // Prepended here (before the `needs_context` wrapper) so it lands INSIDE the
    // wrapper when there is one, at the component-body top.
    let body = if env.uses_stores {
        let decl = env.b.store_subs_var();
        let mut with_subs: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        with_subs.push(decl);
        with_subs.extend_from_slice(body);
        with_subs.into_bump_slice()
    } else {
        body
    };

    // A `$props.id()` declarator hoists `const <name> = $.props_id($$renderer)` to
    // the FIRST statement of the component body (the oracle's
    // `component_block.body.unshift`, placed for hydration). Prepended here — after
    // `out.finish`, before the `needs_context` wrapper — so it lands INSIDE the
    // wrapper when there is one, and before the `$$slots` sanitize decl (which is
    // prepended OUTSIDE the wrapper below). It references `$$renderer` (always in
    // scope), never `$$props`, so it forces no parameter.
    let body = if let Some(name) = &props_id {
        let decl = build_props_id_decl(&mut env.b, arena, name);
        let mut with_id: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
        with_id.push(decl);
        with_id.extend_from_slice(body);
        with_id.into_bump_slice()
    } else {
        body
    };

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

    // A selector chain that matched no element would be pruned by the oracle —
    // pruning isn't implemented, so refuse rather than emit unpruned CSS. The used
    // flags were computed upfront (`match_scope`); the check stays post-emission so
    // an emission refusal keeps priority over it.
    if let Some(scope) = &env.scope
        && let Some(reason) = scope.unused_selectors().next()
    {
        return Err(unsupported(reason));
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

    // The module `<script>` body prints as its own comment-free module-scope
    // program (imports + declarations + non-default exports, source order),
    // between the hoisted snippets and the component function — the oracle's
    // placement (the whole module block follows the hoisted snippets, NOT merged
    // into the instance import group). Empty when there is no module script.
    // Comment-free: the oracle drops module-script comments, so `comments:
    // Vec::new()` reproduces the drop (`format_canonical` prints from the program's
    // comment list, not by positional source lookup — the import program's model).
    let module_program = (!module_body.is_empty()).then(|| tsv_ts::ast::internal::Program {
        body: module_body,
        comments: Vec::new(),
        span: Span::new(0, env.b.buffer.len() as u32),
        interner: std::rc::Rc::clone(&root.interner),
        goal: tsv_ts::Goal::Module,
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
            module_program.as_ref().map_or(&[][..], |p| p.body),
            export_program.body,
        ],
    )?;

    // One caller-owned doc arena shared across the three canonical reprints. The
    // import / hoisted-snippet / export programs are three disjoint sub-ASTs of
    // the same `env.b.buffer` source, so no `reset()` is needed between them, and
    // the arena is a working buffer that never holds content — byte-identical to
    // three fresh-arena `format_canonical` calls. `env.b.buffer` is not mutated
    // between the calls, so one immutable borrow spans all three.
    let doc_arena = tsv_lang::doc::arena::DocArena::for_source(&env.b.buffer);
    let mut js = tsv_ts::format_canonical_in(&import_program, &env.b.buffer, &doc_arena);
    if let Some(hoisted_program) = &hoisted_program {
        js.push_str(&tsv_ts::format_canonical_in(
            hoisted_program,
            &env.b.buffer,
            &doc_arena,
        ));
    }
    if let Some(module_program) = &module_program {
        js.push_str(&tsv_ts::format_canonical_in(
            module_program,
            &env.b.buffer,
            &doc_arena,
        ));
    }
    js.push_str(&tsv_ts::format_canonical_in(
        &export_program,
        &env.b.buffer,
        &doc_arena,
    ));
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

/// `$.bind_props($$props, { … })` — the oracle's trailing bindable-prop
/// registration, appended as the component body's last statement. The object
/// lists the bindable props in source order (shorthand `{ value }` when the prop
/// key equals the local name, `{ value: v }` when renamed).
fn build_bind_props_stmt<'arena>(
    b: &mut Builder<'arena>,
    arena: &'arena bumpalo::Bump,
    entries: &[BindableEntry],
) -> Statement<'arena> {
    let object = build_bindable_object(b, arena, entries);
    let props_ident = Expression::Identifier(b.ident("$$props"));
    let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    args.push(props_ident);
    args.push(object);
    let call = b.member_call("$", "bind_props", args.into_bump_slice());
    let span = call.span();
    Statement::ExpressionStatement(ExpressionStatement {
        expression: call,
        span,
        is_directive: false,
    })
}

/// The `{ key: local, … }` object literal of a `$.bind_props(…)` call. Every
/// node is minted into the appendix (like [`build_sanitize_slots_decl`] and the
/// spread object builders), so no host comment window can be swept; the printer
/// reprints it canonically, so the minted spacing/commas here are cosmetic.
fn build_bindable_object<'arena>(
    b: &mut Builder<'arena>,
    arena: &'arena bumpalo::Bump,
    entries: &[BindableEntry],
) -> Expression<'arena> {
    let obrace = b.mint("{ ").start;
    let mut properties: BumpVec<'arena, ObjectProperty<'arena>> = BumpVec::new_in(arena);
    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            b.mint(", ");
        }
        let shorthand = entry.key == entry.local;
        let key = b.ident(&entry.key);
        let key_span = key.span;
        // Object-shorthand `{ value }` when the key equals the local; otherwise
        // `{ value: v }` with a distinct value identifier.
        let value = if shorthand {
            Expression::Identifier(b.ident(&entry.local))
        } else {
            b.mint(": ");
            Expression::Identifier(b.ident(&entry.local))
        };
        properties.push(ObjectProperty::Property(init_property(
            Expression::Identifier(key),
            value,
            shorthand,
            key_span,
        )));
    }
    let cbrace = b.mint(" }").end;
    Expression::ObjectExpression(ObjectExpression {
        properties: properties.into_bump_slice(),
        spread_trailing_comma: false,
        span: Span::new(obrace, cbrace),
    })
}

/// `const <name> = $.props_id($$renderer);` — the oracle's hoisted `$props.id()`
/// binding, prepended as the component body's first statement.
fn build_props_id_decl<'arena>(
    b: &mut Builder<'arena>,
    arena: &'arena bumpalo::Bump,
    name: &str,
) -> Statement<'arena> {
    // Mint the id before the init so the declaration span runs forward, the same
    // invariant `build_sanitize_slots_decl` relies on.
    let id = Expression::Identifier(b.ident(name));
    let renderer_ident = b.ident("$$renderer");
    let renderer_arg = arena.alloc(Expression::Identifier(renderer_ident));
    let init = b.member_call("$", "props_id", std::slice::from_ref(renderer_arg));
    declaration_stmt(b, VariableDeclarationKind::Const, id, init)
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
