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
    Attribute, AttributeNode, AttributeValue, Element, ExpressionTag, Fragment, FragmentNode,
    HtmlTag, Root, Style,
};
use tsv_ts::ast::internal::{
    BlockStatement, ExportDefaultDeclaration, ExportDefaultValue, Expression, ExpressionStatement,
    FunctionDeclaration, Statement, VariableDeclaration, VariableDeclarator,
};

use crate::analyze::{
    Binding, BindingKind, Bindings, Initial, NameSet, RuneInit, classify_rune_init, evaluate,
    is_effect_call, pattern_binding_names, stringify_value,
};
use crate::build::{Builder, escape_template_text};
use crate::rune_guard::{WalkCtx, walk_expression_guarded, walk_statement_guarded};
use crate::{CompileError, CompileOutput};

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
}

/// Compile a parsed component to server output.
pub(crate) fn compile_server<'arena>(
    root: &Root<'arena>,
    source: &str,
    arena: &'arena bumpalo::Bump,
) -> Result<CompileOutput, CompileError> {
    let mut b = Builder::new(arena, source, std::rc::Rc::clone(&root.interner));

    if root.module.is_some() {
        return Err(unsupported("module <script context=\"module\">"));
    }
    if root.options.is_some() {
        return Err(unsupported("<svelte:options>"));
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
        return Err(unsupported(
            "comments in a script that uses $derived (not carried through yet)",
        ));
    }

    // 4. Script rewrite pass: rune rewrites, guard walks, mutation/shadow
    // collection, effect detection. Rewrites drop source regions (rune call
    // wrappers, whole effect statements) — a comment inside a dropped region
    // has nowhere to go, so it refuses.
    let mut body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    let mut uses_props = false;
    let mut has_effects = false;
    let mut updated = NameSet::default();
    let mut nested_declared = NameSet::default();
    let mut dropped_regions: Vec<Span> = Vec::new();
    if let Some(script) = root.instance {
        for stmt in script.content.body {
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
                &mut dropped_regions,
            )?;
            if let Some(rewritten) = rewritten {
                body.push(rewritten);
            }
        }
    }
    for name in &updated {
        bindings.mark_updated(name);
    }
    for name in &nested_declared {
        bindings.mark_opaque(name);
    }
    for comment in &script_comments {
        for region in &dropped_regions {
            if comment.span.start >= region.start && comment.span.end <= region.end {
                return Err(unsupported(
                    "comment inside a rewritten rune region (dropped by the transform)",
                ));
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
                return Err(unsupported(format!(
                    "css selector .{class} matches no element (pruning not implemented)"
                )));
            }
        }
    }

    // 7. Effects force the `$$renderer.component(($$renderer) => { … })`
    // wrapper around the whole body (the effects themselves are dropped).
    let body = if has_effects {
        let inner_span = Span::new(block_start, env.b.buffer.len() as u32);
        let arrow = env.b.arrow_block("$$renderer", body, inner_span);
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

    let mut import_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    import_body.push(Statement::ImportDeclaration(import));
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
        return Err(unsupported(
            "template comments (only instance-script comments are carried through)",
        ));
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
    let mut comments = Vec::with_capacity(root.comments.len());
    for comment in &root.comments {
        if comment.span.start < content.start || comment.span.end > content.end {
            return Err(unsupported(
                "template comments (only instance-script comments are carried through)",
            ));
        }
        if comment.span.start >= last_stmt_end {
            return Err(unsupported(
                "comment after the last script statement (the oracle re-attaches it into the template)",
            ));
        }
        let text = comment.content(source);
        if text.contains("prettier-ignore") || text.contains("format-ignore") {
            return Err(unsupported("format-ignore directive comment in script"));
        }
        comments.push(comment.clone());
    }
    for node in root.fragment.nodes {
        if node.span().start < content.end {
            return Err(unsupported(
                "comments with template markup before the script (window ordering)",
            ));
        }
    }
    Ok(comments)
}

fn unsupported(what: impl Into<String>) -> CompileError {
    CompileError::Unsupported(what.into())
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
                .ok_or_else(|| unsupported("destructuring a $state declarator"))?;
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
                .ok_or_else(|| unsupported("destructuring a $derived declarator"))?;
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
                .ok_or_else(|| unsupported("destructuring a $derived.by declarator"))?;
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

        let new_init = match rune {
            Some(RuneInit::Props) => {
                *uses_props = true;
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
                        return Err(unsupported(
                            "comments in a script with an argument-less $state()",
                        ));
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
            id: declarator.id.clone(),
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

/// A fragment child after comment-dropping and text decoding, mutable for the
/// whitespace normalization pass.
enum CleanNode<'arena> {
    Text(String),
    Expr(&'arena ExpressionTag<'arena>),
    Html(&'arena HtmlTag<'arena>),
    Element(&'arena Element<'arena>),
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
    /// The component's root fragment (drives the `<!---->` text-first marker).
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

    // Decode and filter into the working list (comments are dropped — the
    // oracle compiles with preserveComments off).
    let mut list: Vec<CleanNode<'arena>> = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            FragmentNode::Text(text) => {
                list.push(CleanNode::Text(text.data(source).into_owned()));
            }
            FragmentNode::Element(element) => list.push(CleanNode::Element(element)),
            FragmentNode::ExpressionTag(tag) => list.push(CleanNode::Expr(tag)),
            FragmentNode::HtmlTag(tag) => list.push(CleanNode::Html(tag)),
            FragmentNode::Comment(_) => {}
            other => {
                return Err(unsupported(format!(
                    "template node {}",
                    fragment_node_kind(other)
                )));
            }
        }
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

    // Component fragment starting with text/{expr}: `<!---->` keeps it from
    // gluing to the previous SSR fragment.
    if ctx.is_component_root
        && matches!(list.first(), Some(CleanNode::Text(_) | CleanNode::Expr(_)))
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
        }
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
    // Guard FIRST (stray runes, non-bare derived reads, template mutations
    // refuse) — the evaluator must never fold an oracle-invalid expression.
    let wrapped = wrap_value_expr(env, expr)?;

    // The fold gate: a known evaluation folds into the static text.
    let evaluated = evaluate(expr, &env.bindings, env.source, 0)
        .map_err(|g| unsupported(format!("static evaluation not portable: {}", g.0)))?;
    if let Some(value) = evaluated.known_value() {
        if !escape {
            // A statically-known `{@html}` would fold through the oracle's html
            // path — not probed/ported, refuse rather than guess.
            return Err(unsupported("{@html} with a statically-known value"));
        }
        let text = stringify_value(value)
            .map_err(|g| unsupported(format!("static fold not portable: {}", g.0)))?;
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
        return Err(unsupported("mutation inside a template expression"));
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
    match name.as_str() {
        // Namespace-dependent whitespace/emission rules not implemented.
        "svg" | "math" => return Err(unsupported(format!("<{name}> (foreign namespace)"))),
        // Template-level <script>/<style> have special semantics in the oracle.
        "script" | "style" => return Err(unsupported(format!("template-level <{name}>"))),
        // The oracle compiles every <option> into `$$renderer.option(…)`
        // closure calls — static markup would be a divergent compile.
        "option" => {
            return Err(unsupported(
                "<option> (oracle emits $$renderer.option closures)",
            ));
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
            return Err(unsupported(format!(
                "<{name}> with children (oracle emits a `<!>` anchor)"
            )));
        }
        _ => {}
    }

    out.push_text(&format!("<{name}"));
    for attr_node in element.attributes {
        let AttributeNode::Attribute(attr) = attr_node else {
            return Err(unsupported("non-plain attribute (directive/spread)"));
        };
        emit_attribute(env, attr, &name, out)?;
    }

    if tsv_html::is_void_element(&name) {
        // XHTML-compliant self-close, matching the oracle.
        out.push_text("/>");
        if !element.fragment.nodes.is_empty() {
            return Err(unsupported(format!("children on void element <{name}>")));
        }
        return Ok(());
    }
    out.push_text(">");
    emit_fragment(
        env,
        &element.fragment,
        out,
        FragmentCtx {
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
    // refuses above).
    let name = env
        .b
        .interner
        .borrow()
        .resolve_infallible(attr.name)
        .to_ascii_lowercase();

    // `value` on <textarea> becomes child content, on <select> it is omitted
    // with select_value bookkeeping — neither shape is implemented.
    if name == "value" && (element_name == "textarea" || element_name == "select") {
        return Err(unsupported(format!("value attribute on <{element_name}>")));
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
            emit_dynamic_attribute(env, &name, &tag.expression, out)
        }
        _ => emit_mixed_attribute(env, &name, values, out),
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

/// A single-expression attribute value: `title={expr}`.
fn emit_dynamic_attribute<'arena>(
    env: &mut EmitEnv<'arena, '_>,
    name: &str,
    expr: &'arena Expression<'arena>,
    out: &mut BodyBuilder<'arena>,
) -> Result<(), CompileError> {
    // The oracle omits expression-valued event handlers from SSR output —
    // implement nothing rather than emit a wrong attribute.
    if name.starts_with("on") {
        return Err(unsupported(format!("event attribute {name}")));
    }
    // The `$.attr` family interleaves minted (appendix) and borrowed (host)
    // argument spans; with host comments present their windows would sweep.
    if env.has_comments {
        return Err(unsupported(
            "comments in a script alongside expression-valued attributes",
        ));
    }
    // A string-literal expression value takes the oracle's inline-literal path
    // (pre-escaped static emission) — refuse rather than guess its edge rules.
    if matches!(expr, Expression::Literal(lit)
        if matches!(lit.value, tsv_ts::ast::internal::LiteralValue::String(_)))
    {
        return Err(unsupported(
            "string-literal expression attribute value (inline-literal path)",
        ));
    }

    let wrapped = wrap_value_expr(env, expr)?;
    let call = match name {
        // Dynamic class/style interact with CSS scoping (hash argument,
        // pruning) — supported only on unstyled components.
        "class" => {
            if env.scope.is_some() {
                return Err(unsupported("dynamic class attribute on a styled component"));
            }
            let clsx = env.b.member_call("$", "clsx", wrapped);
            let clsx_alloc = env.b.arena.alloc(clsx);
            env.b
                .member_call("$", "attr_class", std::slice::from_ref(clsx_alloc))
        }
        "style" => {
            if env.scope.is_some() {
                return Err(unsupported("dynamic style attribute on a styled component"));
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
    if name.starts_with("on") {
        return Err(unsupported(format!("event attribute {name}")));
    }
    if env.has_comments {
        return Err(unsupported(
            "comments in a script alongside expression-valued attributes",
        ));
    }
    if (name == "class" || name == "style") && env.scope.is_some() {
        return Err(unsupported(format!(
            "interpolated {name} attribute on a styled component"
        )));
    }
    let trim_whitespace = name == "class" || name == "style";

    let mut texts: Vec<String> = vec![String::new()];
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
                let evaluated = evaluate(&tag.expression, &env.bindings, env.source, 0)
                    .map_err(|g| unsupported(format!("static evaluation not portable: {}", g.0)))?;
                if let Some(value) = evaluated.known_value() {
                    // Folds into the quasi — plain `(value ?? '') + ''`, no
                    // HTML escaping in the template-value path.
                    let text = stringify_value(value)
                        .map_err(|g| unsupported(format!("static fold not portable: {}", g.0)))?;
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
            return Err(unsupported("css at-rule in <style>"));
        };
        for child in rule.declarations {
            if matches!(child, CssBlockChild::Rule(_) | CssBlockChild::Atrule(_)) {
                return Err(unsupported("nested css rule in <style>"));
            }
        }
        for complex in rule.selector.selectors {
            let [relative] = complex.children else {
                return Err(unsupported("css combinator selector in <style>"));
            };
            let [SimpleSelector::Class { span }] = relative.selectors else {
                return Err(unsupported(
                    "non-class css selector in <style> (only `.class` is supported)",
                ));
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
