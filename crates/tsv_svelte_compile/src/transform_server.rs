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
//! Codegen owns zero precedence knowledge — the printer's `needs_parens`
//! handles it. Shapes the transform does not yet cover return a clear
//! [`CompileError::Unsupported`] rather than guessing.

use std::collections::BTreeSet;

use bumpalo::collections::Vec as BumpVec;
use tsv_css::ast::internal::{CssBlockChild, CssNode, SimpleSelector};
use tsv_lang::{InfallibleResolve, Span};
use tsv_svelte::ast::internal::{
    Attribute, AttributeNode, AttributeValue, Element, Fragment, FragmentNode, Root, Style,
};
use tsv_ts::ast::internal::{
    BlockStatement, Expression, ExportDefaultDeclaration, ExportDefaultValue, ExpressionStatement,
    FunctionDeclaration, Statement, VariableDeclaration, VariableDeclarator,
};

use crate::build::{Builder, escape_template_text};
use crate::{CompileError, CompileOutput};

/// The deterministic scoping class — the fixed `cssHash` the oracle sidecar
/// compiles with, so outputs are byte-comparable across runs.
const SCOPE_HASH_CLASS: &str = "svelte-tsvhash";

/// The component function name. Derived from the constant filename the
/// deterministic oracle compiles under (`input.svelte` → `Input`).
const COMPONENT_NAME: &str = "Input";

/// Compile a parsed component to server output.
pub(crate) fn compile_server<'arena>(
    root: &Root<'arena>,
    source: &str,
    arena: &'arena bumpalo::Bump,
) -> Result<CompileOutput, CompileError> {
    let mut b = Builder::new(arena, source, root.interner.clone());

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
        // TODO: user script comments are dropped for now (the synthetic program
        // carries an empty comment list); carrying them through is a later slice.
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
        true,
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
        interner: root.interner.clone(),
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
    fn push_text(&mut self, text: &str) {
        // Every element of `texts` exists by construction (starts with one entry;
        // `push_expr` appends the follower).
        #[allow(clippy::unwrap_used)]
        self.texts.last_mut().unwrap().push_str(text);
    }

    fn push_expr(&mut self, expr: Expression<'arena>) {
        self.exprs.push(expr);
        self.texts.push(String::new());
    }
}

/// Walk a fragment, appending static HTML to the template text and wrapping
/// `{expr}` interpolations in `$.escape(…)`. At the root boundary,
/// whitespace-only text nodes are trimmed (matching the oracle's SSR output).
fn emit_fragment<'arena>(
    b: &mut Builder<'arena>,
    fragment: &Fragment<'arena>,
    source: &str,
    scope: Option<&ScopeInfo>,
    matched_classes: &mut BTreeSet<String>,
    accum: &mut TemplateAccum<'arena>,
    root_boundary: bool,
) -> Result<(), CompileError> {
    let nodes: &'arena [FragmentNode<'arena>] = fragment.nodes;
    let mut start = 0;
    let mut end = nodes.len();
    if root_boundary {
        while start < end && is_ws_only_text(&nodes[start]) {
            start += 1;
        }
        while end > start && is_ws_only_text(&nodes[end - 1]) {
            end -= 1;
        }
    }
    for node in &nodes[start..end] {
        match node {
            FragmentNode::Text(text) => {
                accum.push_text(&escape_template_text(text.raw_span.extract(source)));
            }
            FragmentNode::Element(element) => {
                emit_element(b, element, source, scope, matched_classes, accum)?;
            }
            FragmentNode::ExpressionTag(tag) => {
                // `{expr}` → `${$.escape(expr)}` with the expression BORROWED
                // (host span, prints verbatim through the normal machinery).
                let args = std::slice::from_ref(&tag.expression);
                let escaped = b.member_call("$", "escape", args);
                accum.push_expr(escaped);
            }
            other => {
                return Err(unsupported(format!(
                    "template node {}",
                    fragment_node_kind(other)
                )));
            }
        }
    }
    Ok(())
}

/// Emit one element's open tag, children, and close tag into the template.
fn emit_element<'arena>(
    b: &mut Builder<'arena>,
    element: &Element<'arena>,
    source: &str,
    scope: Option<&ScopeInfo>,
    matched_classes: &mut BTreeSet<String>,
    accum: &mut TemplateAccum<'arena>,
) -> Result<(), CompileError> {
    let name = b
        .interner
        .borrow()
        .resolve_infallible(element.name)
        .to_string();

    accum.push_text(&format!("<{name}"));
    for attr_node in element.attributes {
        let AttributeNode::Attribute(attr) = attr_node else {
            return Err(unsupported("non-plain attribute (directive/spread)"));
        };
        emit_attribute(b, attr, source, scope, matched_classes, accum)?;
    }
    accum.push_text(">");

    if tsv_html::is_void_element(&name) {
        if !element.fragment.nodes.is_empty() {
            return Err(unsupported(format!("children on void element <{name}>")));
        }
        return Ok(());
    }
    emit_fragment(
        b,
        &element.fragment,
        source,
        scope,
        matched_classes,
        accum,
        false,
    )?;
    accum.push_text(&format!("</{name}>"));
    Ok(())
}

/// Emit one plain attribute (` name` or ` name="value"`), appending the scope
/// hash class to a matched `class` attribute.
fn emit_attribute<'arena>(
    b: &mut Builder<'arena>,
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
        accum.push_text(&format!(" {name}"));
        return Ok(());
    };
    let [AttributeValue::Text(text)] = values else {
        return Err(unsupported(format!(
            "non-static value for attribute {name}"
        )));
    };
    let raw = text.raw_span.extract(source);
    if raw.contains('"') {
        return Err(unsupported(format!(
            "double quote in attribute value {name}"
        )));
    }

    let mut value = raw.to_string();
    if name == "class"
        && let Some(scope) = scope
    {
        let mut matched = false;
        for class in raw.split_ascii_whitespace() {
            if scope.class_names.contains(&class.to_string()) {
                matched_classes.insert(class.to_string());
                matched = true;
            }
        }
        if matched {
            value.push(' ');
            value.push_str(SCOPE_HASH_CLASS);
        }
    }
    accum.push_text(&escape_template_text(&format!(" {name}=\"{value}\"")));
    Ok(())
}

fn is_ws_only_text(node: &FragmentNode<'_>) -> bool {
    matches!(node, FragmentNode::Text(t) if t.is_ascii_ws_only)
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
/// the `$props()` rune: a declarator initialized by `$props()` has its init
/// replaced with the synthetic `$$props` identifier (and the component function
/// gains the `$$props` parameter). Other rune calls are refused; everything
/// else passes through borrowed.
///
/// Passthrough/rebuild is a *shallow* re-slot: `Statement`/`VariableDeclarator`
/// hold children inline by value, so placing a borrowed statement into the
/// synthetic body clones the wrapper only — children remain shared `&'arena`
/// refs into the parsed AST, and the original wrapper never enters the printed
/// tree (no duplicate spans in what the printer walks).
fn rewrite_script_statement<'arena>(
    b: &mut Builder<'arena>,
    stmt: &'arena Statement<'arena>,
    source: &str,
    uses_props: &mut bool,
) -> Result<Statement<'arena>, CompileError> {
    let Statement::VariableDeclaration(decl) = stmt else {
        return Ok(stmt.clone());
    };
    let mut needs_rewrite = false;
    for declarator in decl.declarations {
        if let Some(init) = &declarator.init
            && let Some(rune) = rune_call_name(init, source)
        {
            if rune == "$props" {
                needs_rewrite = true;
            } else {
                return Err(unsupported(format!("rune {rune}")));
            }
        }
    }
    if !needs_rewrite {
        return Ok(stmt.clone());
    }

    let mut declarations: BumpVec<'arena, VariableDeclarator<'arena>> = BumpVec::new_in(b.arena);
    for declarator in decl.declarations {
        let is_props = declarator
            .init
            .as_ref()
            .and_then(|init| rune_call_name(init, source))
            .is_some_and(|rune| rune == "$props");
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

/// The rune name (`$props`, `$state`, …) when `expr` is a direct call of a
/// `$`-prefixed identifier, else `None`.
///
/// Parsed user identifiers are span-identity (`escaped: None` — the name is the
/// leading `name_len` bytes at the span start), so the name is a direct source
/// slice; an interned (escaped) name can't be a rune.
fn rune_call_name<'s>(expr: &Expression<'_>, source: &'s str) -> Option<&'s str> {
    let Expression::CallExpression(call) = expr else {
        return None;
    };
    let Expression::Identifier(id) = call.callee else {
        return None;
    };
    if id.escaped_name.is_some() {
        return None;
    }
    let start = id.span.start as usize;
    let name = &source[start..start + id.name_len as usize];
    name.starts_with('$').then_some(name)
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
