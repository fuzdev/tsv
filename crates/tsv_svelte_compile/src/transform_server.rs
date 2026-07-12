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
//!   values collapse `[ \t\n\r\f]+` runs to one space and trim.
//! - **Void elements** close with `/>`.
//!
//! Codegen owns zero precedence knowledge — the printer's `needs_parens`
//! handles it. Shapes the transform does not yet cover return a clear
//! [`CompileError::Unsupported`] rather than guessing.

use std::collections::BTreeSet;

use bumpalo::collections::Vec as BumpVec;
use tsv_css::ast::internal::{CssBlockChild, CssNode, SimpleSelector};
use tsv_lang::{InfallibleResolve, Span};
use tsv_svelte::ast::internal::{
    Attribute, AttributeNode, AttributeValue, Element, ExpressionTag, Fragment, FragmentNode, Root,
    Style,
};
use tsv_ts::ast::internal::{
    BlockStatement, ExportDefaultDeclaration, ExportDefaultValue, Expression, ExpressionStatement,
    FunctionDeclaration, Statement, VariableDeclaration, VariableDeclarator,
};

use crate::build::{Builder, escape_template_text};
use crate::rune_guard::{refuse_runes_in_expression, refuse_runes_in_statement};
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

    // 2. Instance script statements, rune-rewritten.
    let mut body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    let mut uses_props = false;
    if let Some(script) = root.instance {
        // TODO: carrying user script comments through to the synthetic program
        // is a later slice — refuse rather than silently drop them.
        if !script.content.comments.is_empty() {
            return Err(unsupported(
                "comments in the instance script (not carried through yet)",
            ));
        }
        for stmt in script.content.body {
            let rewritten = rewrite_script_statement(&mut b, stmt, source, &mut uses_props)?;
            body.push(rewritten);
        }
    }

    // 3. Function header skeleton (minted in reading order).
    let export_start = b.mint("export default function ").start;
    let fn_id = b.ident(COMPONENT_NAME);
    let params_start = b.mint("(").start;
    let mut params: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
    let renderer = b.ident("$$renderer");
    params.push(Expression::Identifier(renderer));
    if uses_props {
        b.mint(", ");
        let props = b.ident("$$props");
        params.push(Expression::Identifier(props));
    }
    let lbrace = b.mint(") {").end - 1;

    // 4. Template → one `$$renderer.push(`…`)` statement.
    let mut accum = TemplateAccum {
        texts: vec![String::new()],
        exprs: BumpVec::new_in(arena),
    };
    let mut matched_classes = BTreeSet::new();
    emit_fragment(
        &mut b,
        &root.fragment,
        source,
        scope.as_ref(),
        &mut matched_classes,
        &mut accum,
        FragmentCtx {
            is_component_root: true,
            preserve_whitespace: false,
            parent_name: None,
        },
    )?;
    if !(accum.exprs.is_empty() && accum.texts.iter().all(String::is_empty)) {
        let template = b.template_literal(&accum.texts, accum.exprs.into_bump_slice());
        let mut args: BumpVec<'arena, Expression<'arena>> = BumpVec::new_in(arena);
        args.push(template);
        let push_call = b.member_call("$$renderer", "push", args.into_bump_slice());
        let span = push_call.span();
        body.push(Statement::ExpressionStatement(ExpressionStatement {
            expression: push_call,
            span,
            is_directive: false,
        }));
    }

    // A scoped selector that matches no element would be pruned by the oracle —
    // pruning isn't implemented, so refuse rather than emit unpruned CSS.
    if let Some(scope) = &scope {
        for class in &scope.class_names {
            if !matched_classes.contains(class) {
                return Err(unsupported(format!(
                    "css selector .{class} matches no element (pruning not implemented)"
                )));
            }
        }
    }

    // 5. Assemble function + program.
    let rbrace_end = b.mint("}").end;
    let function = FunctionDeclaration {
        id: Some(fn_id),
        type_parameters: None,
        params: params.into_bump_slice(),
        return_type: None,
        body: BlockStatement {
            body: body.into_bump_slice(),
            span: Span::new(lbrace, rbrace_end),
        },
        generator: false,
        r#async: false,
        params_start,
        span: Span::new(export_start + "export default ".len() as u32, rbrace_end),
    };
    let export = ExportDefaultDeclaration {
        declaration: ExportDefaultValue::FunctionDeclaration(function),
        span: Span::new(export_start, rbrace_end),
    };

    let mut program_body: BumpVec<'arena, Statement<'arena>> = BumpVec::new_in(arena);
    program_body.push(Statement::ImportDeclaration(import));
    program_body.push(Statement::ExportDefaultDeclaration(export));
    let program = tsv_ts::ast::internal::Program {
        body: program_body.into_bump_slice(),
        comments: Vec::new(),
        span: Span::new(0, b.buffer.len() as u32),
        interner: std::rc::Rc::clone(&root.interner),
        goal: tsv_ts::Goal::Module,
    };

    let js = tsv_ts::format_canonical(&program, &b.buffer);
    let css = match (root.css, &scope) {
        (Some(style), Some(scope)) => Some(splice_scoped_css(style, source, scope)),
        _ => None,
    };

    Ok(CompileOutput {
        js,
        css,
        warnings: Vec::new(),
    })
}

fn unsupported(what: impl Into<String>) -> CompileError {
    CompileError::Unsupported(what.into())
}

/// Alternating static template text and interpolation expressions
/// (`texts.len() == exprs.len() + 1` — the [`Builder::template_literal`] shape).
struct TemplateAccum<'arena> {
    texts: Vec<String>,
    exprs: BumpVec<'arena, Expression<'arena>>,
}

impl<'arena> TemplateAccum<'arena> {
    /// Append an already template-escaped chunk to the current static part.
    ///
    /// Cross-chunk `${` seam invariant: each chunk is template-escaped
    /// independently, so a literal `$` ending one chunk followed by a literal
    /// `{` starting the next would slip through unescaped. That pairing is
    /// unreachable — a decoded text run is always a single chunk (the parser
    /// yields one `Text` node per run, entities included), and every other
    /// chunk this transform appends starts with `<`, `/`, `>`, or a space —
    /// but assert it so a future emitter change fails loudly.
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
    Element(&'arena Element<'arena>),
}

impl CleanNode<'_> {
    fn is_expr(&self) -> bool {
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
/// then append static HTML / `$.escape(expr)` interpolations to the template.
fn emit_fragment<'arena>(
    b: &mut Builder<'arena>,
    fragment: &Fragment<'arena>,
    source: &str,
    scope: Option<&ScopeInfo>,
    matched_classes: &mut BTreeSet<String>,
    accum: &mut TemplateAccum<'arena>,
    ctx: FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let nodes: &'arena [FragmentNode<'arena>] = fragment.nodes;

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
        accum.push_text("<!---->");
    }

    for node in &list {
        match node {
            CleanNode::Text(data) => {
                accum.push_text(&escape_template_text(&escape_html_text(data)));
            }
            CleanNode::Element(element) => {
                emit_element(b, element, source, scope, matched_classes, accum, &ctx)?;
            }
            CleanNode::Expr(tag) => {
                // Runes are script-only; refuse them in template expressions too.
                refuse_runes_in_expression(&tag.expression, source)?;
                // `{expr}` → `${$.escape(expr)}` with the expression BORROWED
                // (host span, prints verbatim through the normal machinery).
                let args = std::slice::from_ref(&tag.expression);
                let escaped = b.member_call("$", "escape", args);
                accum.push_expr(escaped);
            }
        }
    }
    Ok(())
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
    b: &mut Builder<'arena>,
    element: &'arena Element<'arena>,
    source: &str,
    scope: Option<&ScopeInfo>,
    matched_classes: &mut BTreeSet<String>,
    accum: &mut TemplateAccum<'arena>,
    parent_ctx: &FragmentCtx<'_>,
) -> Result<(), CompileError> {
    let name = b
        .interner
        .borrow()
        .resolve_infallible(element.name)
        .to_string();
    match name.as_str() {
        // Namespace-dependent whitespace/emission rules not implemented.
        "svg" | "math" => return Err(unsupported(format!("<{name}> (foreign namespace)"))),
        // Template-level <script>/<style> have special semantics in the oracle.
        "script" | "style" => return Err(unsupported(format!("template-level <{name}>"))),
        _ => {}
    }

    accum.push_text(&format!("<{name}"));
    for attr_node in element.attributes {
        let AttributeNode::Attribute(attr) = attr_node else {
            return Err(unsupported("non-plain attribute (directive/spread)"));
        };
        emit_attribute(b, attr, source, scope, matched_classes, accum)?;
    }

    if tsv_html::is_void_element(&name) {
        // XHTML-compliant self-close, matching the oracle.
        accum.push_text("/>");
        if !element.fragment.nodes.is_empty() {
            return Err(unsupported(format!("children on void element <{name}>")));
        }
        return Ok(());
    }
    accum.push_text(">");
    emit_fragment(
        b,
        &element.fragment,
        source,
        scope,
        matched_classes,
        accum,
        FragmentCtx {
            is_component_root: false,
            preserve_whitespace: parent_ctx.preserve_whitespace
                || name == "pre"
                || name == "textarea",
            parent_name: Some(&name),
        },
    )?;
    accum.push_text(&format!("</{name}>"));
    Ok(())
}

/// Emit one plain attribute: ` name=""` for a boolean attribute, else
/// ` name="decoded value"` with the oracle's attribute escaping; `class`/`style`
/// values collapse whitespace runs and trim, and a matched `class` gains the
/// scope hash class.
fn emit_attribute<'arena>(
    b: &Builder<'arena>,
    attr: &Attribute<'arena>,
    source: &str,
    scope: Option<&ScopeInfo>,
    matched_classes: &mut BTreeSet<String>,
    accum: &mut TemplateAccum<'arena>,
) -> Result<(), CompileError> {
    let name = b
        .interner
        .borrow()
        .resolve_infallible(attr.name)
        .to_string();

    let Some(values) = attr.value else {
        // Boolean attribute: the oracle emits `name=""`.
        accum.push_text(&escape_template_text(&format!(" {name}=\"\"")));
        return Ok(());
    };
    let [AttributeValue::Text(text)] = values else {
        return Err(unsupported(format!(
            "non-static value for attribute {name}"
        )));
    };
    let decoded = text.data(source);

    // class/style are whitespace-insensitive: runs ([ \t\n\r\f]+) collapse to
    // one space and the whole value trims (the oracle's
    // WHITESPACE_INSENSITIVE_ATTRIBUTES handling).
    let mut value = if name == "class" || name == "style" {
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
    } else {
        decoded.into_owned()
    };

    if name == "class"
        && let Some(scope) = scope
    {
        let mut matched = false;
        for class in value.split_ascii_whitespace() {
            if scope.class_names.contains(class) {
                matched_classes.insert(class.to_string());
                matched = true;
            }
        }
        if matched {
            value.push(' ');
            value.push_str(SCOPE_HASH_CLASS);
        }
    }
    accum.push_text(&escape_template_text(&format!(
        " {name}=\"{}\"",
        escape_html_attr(&value)
    )));
    Ok(())
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

/// Rewrite one instance-script statement for the server module. Today that is
/// the `$props()` rune: a top-level declarator initialized by a direct,
/// argument-less `$props()` call has its init replaced with the synthetic
/// `$$props` identifier (and the component function gains the `$$props`
/// parameter). Every other rune use anywhere in the statement — statement
/// position, nested functions, member-form calls — is refused by the
/// `rune_guard` walk; everything else passes through borrowed.
///
/// Passthrough/rebuild is a *shallow* re-slot: `Statement`/`VariableDeclarator`
/// hold children inline by value, so placing a borrowed statement into the
/// synthetic body clones the wrapper only — children remain shared `&'arena`
/// refs into the parsed AST, and the original wrapper never enters the printed
/// tree (no duplicate spans in what the printer walks). See `build.rs` for the
/// address-keyed side-table caveat.
fn rewrite_script_statement<'arena>(
    b: &mut Builder<'arena>,
    stmt: &'arena Statement<'arena>,
    source: &str,
    uses_props: &mut bool,
) -> Result<Statement<'arena>, CompileError> {
    let Statement::VariableDeclaration(decl) = stmt else {
        refuse_runes_in_statement(stmt, source)?;
        return Ok(stmt.clone());
    };

    let has_props_init = decl
        .declarations
        .iter()
        .any(|d| d.init.as_ref().is_some_and(|i| is_props_call(i, source)));
    if !has_props_init {
        refuse_runes_in_statement(stmt, source)?;
        return Ok(stmt.clone());
    }

    let mut declarations: BumpVec<'arena, VariableDeclarator<'arena>> = BumpVec::new_in(b.arena);
    for declarator in decl.declarations {
        // The binding pattern is guarded in every case (a rune can't hide in a
        // pattern default either).
        refuse_runes_in_expression(&declarator.id, source)?;
        let is_props = declarator
            .init
            .as_ref()
            .is_some_and(|init| is_props_call(init, source));
        if is_props {
            *uses_props = true;
            let props_ident = b.ident("$$props");
            declarations.push(VariableDeclarator {
                id: declarator.id.clone(),
                init: Some(Expression::Identifier(props_ident)),
                definite: declarator.definite,
                span: declarator.span,
            });
        } else {
            if let Some(init) = &declarator.init {
                refuse_runes_in_expression(init, source)?;
            }
            declarations.push(declarator.clone());
        }
    }
    Ok(Statement::VariableDeclaration(VariableDeclaration {
        kind: decl.kind,
        declarations: declarations.into_bump_slice(),
        declare: decl.declare,
        span: decl.span,
    }))
}

/// Whether `expr` is exactly the sanctioned rewrite shape: a direct,
/// argument-less call of the plain identifier `$props`.
fn is_props_call(expr: &Expression<'_>, source: &str) -> bool {
    let Expression::CallExpression(call) = expr else {
        return false;
    };
    if !call.arguments.is_empty() {
        return false;
    }
    let Expression::Identifier(id) = call.callee else {
        return false;
    };
    if id.escaped_name.is_some() {
        return false;
    }
    let start = id.span.start as usize;
    &source[start..start + id.name_len as usize] == "$props"
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
