//! Instance-script analysis and rewrite for the server transform.
//!
//! The document-wide TypeScript flag and gate, the top-level binding-table
//! analysis, the per-statement rune rewrites (`$props()` → `$$props`,
//! `$state`/`$derived` unwrap, dropped $effect), and the erase self-check that
//! closes the loop on the finished program. See [`crate::transform_server`] for
//! the orchestration that calls these in sequence.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{InfallibleResolve, SharedInterner, Span};
use tsv_svelte::ast::internal::{AttributeNode, AttributeValue, ElementKind, FragmentNode, Root};
use tsv_ts::ast::internal::{
    AssignmentPattern, ClassBody, ClassDeclaration, ClassMember, Expression, ImportDeclaration,
    ImportSpecifier, LiteralValue, ModuleExportName, ObjectPattern, ObjectPatternProperty,
    Property, PropertyDefinition, PropertyKind, RestElement, Statement, VariableDeclaration,
    VariableDeclarator,
};

use crate::analyze::{
    Binding, BindingKind, Bindings, Initial, NameSet, RUNE_BASES, RuneInit, classify_rune_init,
    is_effect_call, is_inspect_call, pattern_binding_names, pattern_binds_unnameable_identifier,
};
use crate::attr_refs::{TemplateItem, each_template_item};
use crate::build::Builder;
use crate::fragment::is_bare_derived_read;
use crate::rune_guard::{
    WalkCtx, refuse_dollar_binding_name, refuse_dollar_binding_pattern,
    refuse_dollar_import_locals, walk_class_member_guarded, walk_expression_guarded,
    walk_statement_guarded,
};
use crate::text_class::is_js_whitespace;
use crate::transform_server::unsupported;
use crate::{CompileError, Refusal, erase};

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
///
/// A comment inside the **module** script's content span is DROPPED (skipped, not
/// carried and not refused): the oracle drops every module-script comment, so
/// emitting the module body comment-free reproduces the drop as parity.
pub(crate) fn collect_script_comments(
    root: &Root<'_>,
    source: &str,
    instance_body: &[Statement<'_>],
) -> Result<Vec<tsv_lang::Comment>, CompileError> {
    if root.comments.is_empty() {
        return Ok(Vec::new());
    }
    // The oracle drops module-script comments — a comment fully within the module
    // content span never carries and never refuses.
    let module_content = root.module.map(|module| module.content.span);
    let in_module = |comment: &tsv_lang::Comment| {
        module_content.is_some_and(|m| comment.span.start >= m.start && comment.span.end <= m.end)
    };
    let Some(script) = root.instance else {
        // No instance script to carry into: any comment that is not a (dropped)
        // module comment is a template comment we don't thread — refuse.
        for comment in &root.comments {
            if !in_module(comment) {
                return Err(unsupported(Refusal::TemplateComments));
            }
        }
        return Ok(Vec::new());
    };
    let content = script.content.span;
    // A comment at or past the last SURVIVING statement has no statement left to
    // lead — an `import` hoists to the comment-free module program and a
    // statement-position `$effect`/`$inspect` drops, so neither anchors one. The
    // bound is the last surviving statement's end, `content.start` when nothing
    // survives (an import-only script).
    //
    // Such a comment still carries: the oracle re-attaches it into the template
    // (trailing the final push, or nested inside the next emitted node — an
    // `{#if}` condition, an `$.ensure_array_like(…)` / `$.attr(…)` argument) while
    // tsv's printer lands it at the end of the synthetic function body (the body
    // block's span runs `[content.start, rbrace_end)`, so the block's trailing
    // window captures it exactly once). The placements differ, but the parity bar
    // grades comment DROP / COUNT / CONTENT, not position.
    //
    // The one shape that does NOT converge is a template emitting a nested block —
    // see [`template_emits_nested_block`].
    let survives = |stmt: &Statement<'_>| match stmt {
        Statement::ImportDeclaration(_) => false,
        Statement::ExpressionStatement(expr_stmt) => {
            is_effect_call(&expr_stmt.expression, source).is_none()
                && is_inspect_call(&expr_stmt.expression, source).is_none()
        }
        _ => true,
    };
    let last_stmt_end = instance_body
        .iter()
        .filter(|stmt| survives(stmt))
        .map(|stmt| stmt.span().end)
        .max()
        .unwrap_or(content.start);
    let nested_block = template_emits_nested_block(root.fragment.nodes);
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
        // Module-script comments drop (the oracle drops them); the module body
        // emits comment-free, so skipping here reproduces the drop as parity.
        if in_module(comment) {
            continue;
        }
        if comment.span.start < content.start || comment.span.end > content.end {
            return Err(unsupported(Refusal::TemplateComments));
        }
        // A multi-line block comment carries verbatim, but the oracle (esrap)
        // re-indents its interior lines to the emit position, so the two diverge on
        // any interior line whose source indentation differs from the target — refuse
        // until the printer re-indents block-comment interiors to match. Checked
        // before the after-last rule below so this independent gate keeps its own
        // refusal bucket whatever the template emits.
        if comment.multiline {
            return Err(unsupported(Refusal::MultilineBlockComment));
        }
        if nested_block && comment.span.start >= last_stmt_end {
            return Err(unsupported(Refusal::CommentAfterLastStatementWithBlock));
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
        // A whitespace-only text node — e.g. the run between a module `</script>`
        // and the instance `<script>`, or leading/trailing template whitespace —
        // is not real markup, so it doesn't force the refusal. Any genuine
        // element / expression / comment / block before the instance script's end
        // still refuses (its emitter's comment window would sweep the carried
        // script comments). A Unicode-whitespace-only text (`is_ascii_ws_only ==
        // false`) is content and correctly still refuses.
        if let FragmentNode::Text(text) = node
            && text.is_ascii_ws_only
        {
            continue;
        }
        if node.span().start < content.end {
            return Err(unsupported(Refusal::CommentsWithTemplateBeforeScript));
        }
    }
    Ok(comments)
}

/// Does the template emit a **synthetic block** — a `{ … }` body the oracle
/// builds with no source `loc`?
///
/// This decides whether a comment past the last surviving script statement can be
/// carried. The oracle's printer (esrap) walks one `comment_index` over the comment
/// list, and `body()` opens every block with `reset_comment_index(node)`. That reset
/// has two arms: a block with **no** `loc` sets the index to `comments.length`,
/// **discarding every comment not yet written**; a block that **has** a `loc`
/// re-seeks the index absolutely (`comments.findIndex(…)`), which can move it
/// **backward**. So a loc-less block annihilates the index and the next loc-bearing
/// block **recovers** it.
///
/// That recovery — not an exemption — is what carries an after-last comment through
/// to the component body. The body block is assigned the instance script's `loc`
/// (the transform's "trick esrap into including comments" line), and when the
/// component needs a context wrapper the transform **reassigns** `component_block`
/// to a fresh loc-LESS block around that loc-bearing one. The wrapper does annihilate
/// the index; the inner block then seeks back over the comment, so it still reaches
/// the body's closing flush. A template block gets no such recovery — it is loc-less,
/// reached after the body has already seeked, with nothing loc-bearing behind it to
/// seek back — so the comment is DROPPED, a divergence the parity bar grades, unlike
/// a mere position difference.
///
/// The scan is deliberately blunt: it answers "does a synthetic block exist
/// anywhere", not "is one reached before the comment would flush". A loc-bearing
/// expression emitted first (an `{#if}` test, an `{#each}` expression) flushes the
/// comment ahead of the block and the oracle keeps it — so `{#if x}` with an
/// after-last comment converges in practice, and this scan over-refuses it.
/// Tightening that costs an ordered next-emitted-node walk plus the oracle's
/// fold/rewrite rules for which expressions keep a `loc`; a safe over-refusal is
/// preferred to guessing.
///
/// The [`FragmentNode::SpecialElement`] arm is intentionally blanket-TRUE for the
/// same reason. Several kinds do emit a block and genuinely drop the comment
/// (`<svelte:head>`, `<svelte:element>`, `<svelte:boundary>`), but `<svelte:window>`
/// and `<slot>` emit no block at all and are knowingly over-refused — the blanket arm
/// buys a conservative safety margin, not a claim that every kind drops.
///
/// ⚠️ This TRUE/FALSE split is keyed to the **pinned** oracle's `reset_comment_index`
/// behavior (esrap 2.2.12, via the pinned Svelte compiler). If that pin moves, re-probe
/// the split against the new oracle rather than assuming it carries over.
///
/// Exhaustively matched so a new [`FragmentNode`] variant fails compilation here
/// rather than silently defaulting to "no block".
fn template_emits_nested_block(nodes: &[FragmentNode<'_>]) -> bool {
    nodes.iter().any(|node| match node {
        // Leaves and the tags that emit a bare call — no block.
        FragmentNode::Text(_)
        | FragmentNode::Comment(_)
        | FragmentNode::ExpressionTag(_)
        | FragmentNode::HtmlTag(_)
        | FragmentNode::RenderTag(_) => false,
        // Every block/closure emitter: `{#if}`/`{#each}` bodies, the `$.await` and
        // `$.head`/`$.element` closures, a `{#snippet}` function.
        FragmentNode::IfBlock(_)
        | FragmentNode::EachBlock(_)
        | FragmentNode::AwaitBlock(_)
        | FragmentNode::KeyBlock(_)
        | FragmentNode::SnippetBlock(_)
        | FragmentNode::SpecialElement(_)
        // Refused elsewhere; counted here so the scan never under-reports.
        | FragmentNode::ConstTag(_)
        | FragmentNode::DeclarationTag(_)
        | FragmentNode::DebugTag(_) => true,
        FragmentNode::Element(element) => match element.kind {
            // A component's children become a `children: ($$renderer) => { … }`
            // snippet prop — a block. Childless (or whitespace-only), it is a bare
            // `Foo($$renderer, {…})` call.
            ElementKind::Component => {
                element.fragment.nodes.iter().any(|child| {
                    !matches!(child, FragmentNode::Text(text) if text.is_ascii_ws_only)
                })
            }
            ElementKind::Html => template_emits_nested_block(element.fragment.nodes),
        },
    })
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

/// Refuse a `$derived(…)` whose WHOLE argument is a bare `$derived` read
/// (`$derived(d)` where `d` is another derived). The oracle unthunk-collapses it
/// to `$.derived(d)` (the derived function passed straight through, not read), a
/// form the script rewrite can't reproduce — the store rewrite would turn the
/// argument into `d()`, giving `$.derived(() => d())`. A safe over-refusal (the
/// read refused before this slice too). NOT applied to `$derived.by(d)`, whose
/// oracle output IS `$.derived(d())` — reproduced by the rewrite (`.by` runs no
/// `unthunk`), so it compiles.
fn refuse_bare_derived_arg(
    expr: &Expression<'_>,
    source: &str,
    derived_names: &NameSet,
) -> Result<(), CompileError> {
    if let Expression::Identifier(id) = expr
        && is_bare_derived_read(source, derived_names, expr)
        && let Some(name) = plain_identifier_name(id, source)
    {
        return Err(unsupported(Refusal::DerivedBindingRead { name }));
    }
    Ok(())
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
pub(crate) fn self_check_no_typescript<'arena>(
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
/// **Both** top-level scripts are considered, in source order (a `<script
/// module>` can set the flag exactly as an instance script does), mirroring
/// Svelte's single component-wide `this.ts` decision. The FIRST lang-bearing
/// script decides — a later one's `lang` is ignored, so an expression-valued
/// `lang` on it does not refuse. `generics` on *either* script is refused
/// outright (an open type-parameter *binding*, not annotation erasure), as is a
/// deciding `lang` other than `ts`/`js`/empty.
pub(crate) fn document_ts_flag(root: &Root<'_>, source: &str) -> Result<bool, CompileError> {
    // Both scripts in source order — the first lang-bearing one decides.
    let mut scripts = [root.module, root.instance];
    scripts.sort_by_key(|s| s.map_or(u32::MAX, |script| script.span.start));
    let mut ts = false;
    let mut decided = false;
    for script in scripts.into_iter().flatten() {
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
                    // Only the first lang-bearing script decides; a later `lang`
                    // (including an unclassifiable expression-valued one) is
                    // ignored exactly as Svelte's first-match regex ignores it.
                    if decided {
                        continue;
                    }
                    match attr.value {
                        // A bare `lang` (no value) never matches the oracle's
                        // regex — plain JS, like no attribute at all, and it does
                        // NOT count as the deciding script.
                        Some([]) | None => {}
                        Some([AttributeValue::Text(text)]) => {
                            let lang = text.data(source);
                            match lang.as_ref() {
                                "ts" => {
                                    ts = true;
                                    decided = true;
                                }
                                "js" | "" => decided = true,
                                _ => {
                                    return Err(unsupported(Refusal::LangInstanceScript {
                                        lang: lang.into_owned(),
                                    }));
                                }
                            }
                        }
                        // An expression-valued `lang` on the deciding script can't
                        // be classified.
                        _ => {
                            return Err(unsupported(Refusal::LangInstanceScript {
                                lang: String::new(),
                            }));
                        }
                    }
                }
                "generics" => {
                    return Err(unsupported(Refusal::GenericsAttribute));
                }
                _ => {}
            }
        }
    }
    Ok(ts)
}

/// Erase and validate a plain module `<script module>` / `<script
/// context="module">`, returning its type-free statement list (imports +
/// declarations + non-default exports, source order) for module-scope emission.
///
/// v1 supports **plain** module scripts only. TypeScript erases under the
/// document `lang="ts"` flag exactly as the instance script does. Then, per
/// statement:
///
/// - `export default` refuses [`Refusal::ModuleDefaultExport`] — the oracle
///   errors `module_illegal_default_export`;
/// - an invalid runes-mode import (`svelte/internal*`,
///   `beforeUpdate`/`afterUpdate`) refuses via [`refuse_runes_invalid_import`];
/// - the statement is guard-walked **without** a store exemption, so a
///   module-scope rune, a `$name` store read (the oracle's
///   `store_invalid_subscription`), or a top-level `await` refuses — v1 defers
///   the oracle's module `$state`→`v` / `$derived`→`$.derived(…)` rewrites (the
///   corpus is rune-free, so this is a lossless over-refusal).
///
/// A supported module body emits **verbatim** (post-erase): the oracle's
/// module-body reassignment/needs_context effects flow through the shared
/// whole-component analysis ([`crate::needs_context::analyze_component`]) and the
/// binding table ([`analyze_script`]), not through any module-only rewrite.
pub(crate) fn analyze_module_script<'arena>(
    root: &Root<'arena>,
    source: &str,
    arena: &'arena bumpalo::Bump,
    ts_document: bool,
) -> Result<&'arena [Statement<'arena>], CompileError> {
    let Some(script) = root.module else {
        return Ok(&[]);
    };
    let erased = erase::erase_statements(arena, source, script.content.body)?;
    // The same document-wide TypeScript gate the instance body pays: without the
    // flag, a `: T` / `as T` / `x!` in the module is a plain-JS parse error in the
    // oracle, so a permissive accept here would be an over-acceptance.
    if erased.typescript && !ts_document {
        return Err(unsupported(Refusal::TypeScriptWithoutLangTs));
    }
    // Scratch collection sinks — the guard walk's reassignment/shadow collection is
    // redundant here (the whole-component `analyze_component` covers module scope),
    // so only its REFUSAL is wanted. Derived reads are impossible in a module (no
    // module `$derived` survives the guard), so an empty derived set avoids a false
    // `DerivedBindingRead` on a name that merely coincides with an instance derived.
    let mut updated = NameSet::default();
    let mut nested = NameSet::default();
    let derived = NameSet::default();
    for stmt in erased.body {
        if matches!(stmt, Statement::ExportDefaultDeclaration(_)) {
            return Err(unsupported(Refusal::ModuleDefaultExport));
        }
        if let Statement::ImportDeclaration(import) = stmt {
            refuse_runes_invalid_import(import, source)?;
        }
        let mut ctx = WalkCtx::new(
            source,
            &mut updated,
            &mut nested,
            &derived,
            std::rc::Rc::clone(&root.interner),
        );
        walk_statement_guarded(stmt, &mut ctx, 0)?;
    }
    Ok(erased.body)
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
pub(crate) fn refuse_template_typescript<'arena>(
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
pub(crate) fn analyze_script<'arena>(
    stmts: &'arena [Statement<'arena>],
    source: &str,
    bindings: &mut Bindings<'arena>,
    derived_names: &mut NameSet,
) -> Result<(), CompileError> {
    // The oracle's duplicate-`$props()` flag is per-SCRIPT state — its analyze phase
    // seeds a fresh `has_props_rune: false` for the module and the instance analysis
    // alike (`phases/2-analyze/index.js:313,725,793`) — so the flag is scoped to one
    // `analyze_script` call and the export recursion inside it, not to the component.
    let mut seen_props = false;
    analyze_script_in(stmts, source, bindings, derived_names, &mut seen_props)
}

fn analyze_script_in<'arena>(
    stmts: &'arena [Statement<'arena>],
    source: &str,
    bindings: &mut Bindings<'arena>,
    derived_names: &mut NameSet,
    seen_props: &mut bool,
) -> Result<(), CompileError> {
    each_script_declaration(stmts, VarScope::TopLevelOnly, &mut |decl| {
        let (id, initial) = match decl {
            // `VarScope::TopLevelOnly` never reports a hoisted declarator, so
            // `initial_dropped` is always false here.
            ScriptDeclaration::Declarator { declarator, .. } => {
                return analyze_declarator(declarator, source, bindings, derived_names, seen_props);
            }
            ScriptDeclaration::Function(id) => (id, Initial::Function),
            ScriptDeclaration::Class(id) | ScriptDeclaration::Import { local: id, .. } => {
                (id, Initial::None)
            }
        };
        if let Some(name) = plain_identifier_name(id, source) {
            bindings.insert(
                name,
                Binding {
                    kind: BindingKind::Normal,
                    initial,
                    updated: false,
                },
            );
        }
        Ok(())
    })
}

/// Which of a script's `var` declarations [`each_script_declaration`] reports.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum VarScope {
    /// Only the statements at the list's own level — the shape the binding-table
    /// analysis wants (it classifies *top-level declarations*, in source order).
    TopLevelOnly,
    /// Plus every `var` declarator hoisted out of a nested block, for-head, or
    /// other non-function statement. `var` is function-scoped, so such a
    /// declarator lands in the script's own scope exactly like a top-level one —
    /// which is what a "what does this script DECLARE" question must see.
    WithHoistedVars,
}

/// One binding a script's statements introduce at SCRIPT scope.
pub(crate) enum ScriptDeclaration<'arena> {
    /// A `var`/`let`/`const`/`using` declarator.
    Declarator {
        declarator: &'arena VariableDeclarator<'arena>,
        /// Whether this declarator reached script scope by hoisting **through a
        /// POROUS scope** — a block, a for-head/body, a switch, a try/catch. The
        /// oracle re-declares such a `var` on the parent scope and passes NO
        /// initializer while doing so (`phases/scope.js:673-681` —
        /// `return this.parent.declare(node, kind, declaration_kind)`, the
        /// 4-argument `initial` defaulting to `null`), so the binding it creates
        /// reads as having no initializer at all. A declarator at the script's
        /// own level keeps its `initial`.
        initial_dropped: bool,
    },
    /// A `function name(…) {}` declaration's name.
    Function(&'arena tsv_ts::ast::internal::Identifier<'arena>),
    /// A `class Name {}` declaration's name.
    Class(&'arena tsv_ts::ast::internal::Identifier<'arena>),
    /// One `import` specifier's local name, with the declaration it came from —
    /// the oracle's `Binding.initial` for an import binding is the whole
    /// `ImportDeclaration`, and its `source` is what the `svelte/store` carve-out
    /// tests.
    Import {
        local: &'arena tsv_ts::ast::internal::Identifier<'arena>,
        declaration: &'arena ImportDeclaration<'arena>,
    },
}

/// The single answer to **"what does this script declare at script scope?"**.
///
/// Every analysis that needs that answer routes through here. The match is
/// **exhaustive on purpose** — a new `Statement` variant fails compilation rather
/// than being silently skipped, which is exactly how a hand-copied enumeration
/// with a `_ => {}` tail goes stale and starts missing bindings (an
/// under-refusal, i.e. a MISMATCH, for the collision walk below).
pub(crate) fn each_script_declaration<'arena, E>(
    stmts: &'arena [Statement<'arena>],
    var_scope: VarScope,
    f: &mut impl FnMut(ScriptDeclaration<'arena>) -> Result<(), E>,
) -> Result<(), E> {
    for stmt in stmts {
        script_declarations_of(stmt, var_scope, true, false, f)?;
    }
    Ok(())
}

/// `top` distinguishes the script's own statement list from a nested one. Below
/// the top level only a `var` reaches script scope: `let`/`const` are
/// block-scoped, and a nested `function`/`class` declaration is block-scoped too
/// (tsv is strict-mode-only, so Annex B function hoisting does not apply).
///
/// `porous` records whether at least one POROUS scope sits between `stmt` and the
/// script scope, because that changes what the oracle's binding CARRIES — see
/// [`ScriptDeclaration::Declarator`]'s `initial_dropped`. A porous scope is
/// exactly the set of containers `phases/scope.js` gives a `scope.child(true)`:
/// `BlockStatement`, `ForStatement` / `ForInStatement` / `ForOfStatement`,
/// `SwitchStatement` (all four via `create_block_scope`), and a `CatchClause`
/// with a parameter. An `if` / `while` / `do` / `try` statement gets no visitor at
/// all, so it is transparent — probe-verified: `if (x) var state = $state(0);`
/// keeps its initializer (the oracle does NOT reclassify it), while
/// `if (x) { var state = $state(0); }` loses it (the oracle DOES).
///
/// A `LabeledStatement` is transparent here, which is exact for every label but
/// one. The oracle's visitor (`phases/scope.js:1063-1069`) falls through to a bare
/// `next()` unless the label is at `path.length === 1`, is named `$`, AND the
/// script was scoped with `allow_reactive_declarations`. That flag is NOT
/// mode-keyed: `2-analyze/index.js:336-337` passes `false` for the MODULE script
/// and `true` for the INSTANCE script, in runes mode as much as in legacy mode. So
/// a top-level `$:` in the instance script does get a scope — a NON-porous one
/// (`scope.child()`, `porous = false` by default, `scope.js:702`), meaning its
/// `var` should not reach script scope at all, and this walk over-collects it.
/// Harmless: a top-level `$:` is invalid in runes mode and refuses on its own
/// path, so the only effect is which refusal bucket a `$: var state = …` document
/// lands in.
///
/// ⚠️ A **class body is opaque** here, on both the declaration and the expression
/// side. A class static block is the one nested statement list the oracle gives no
/// scope at all (`phases/scope.js` has no `StaticBlock` visitor), so a `var` there
/// really does reach script scope — but reaching every class body a statement can
/// hold means traversing every expression position of every statement, a
/// hand-enumerated surface that has twice shipped with holes. The collision check
/// handles the whole family with a lexical fence instead
/// ([`script_contains_static_block`]), so this walk deliberately stops at a class
/// body and no consumer depends on it descending.
fn script_declarations_of<'arena, E>(
    stmt: &'arena Statement<'arena>,
    var_scope: VarScope,
    top: bool,
    porous: bool,
    f: &mut dyn FnMut(ScriptDeclaration<'arena>) -> Result<(), E>,
) -> Result<(), E> {
    use tsv_ts::ast::internal::{ForInOfLeft, ForInit, VariableDeclarationKind};

    // Descending at all is only ever about collecting hoisted `var`s.
    let descend = matches!(var_scope, VarScope::WithHoistedVars);
    // A statement body that is transparent in the oracle's scope walk.
    macro_rules! nested {
        ($s:expr) => {
            if descend {
                script_declarations_of($s, var_scope, false, porous, f)?;
            }
        };
    }
    // A statement body the oracle wraps in a porous scope.
    macro_rules! nested_porous {
        ($s:expr) => {
            if descend {
                script_declarations_of($s, var_scope, false, true, f)?;
            }
        };
    }
    macro_rules! head_declaration {
        ($decl:expr) => {
            // A for-head is never the script's own statement list, so only `var` —
            // and the for statement's own scope is porous, so the initializer goes.
            if descend && $decl.kind == VariableDeclarationKind::Var {
                for declarator in $decl.declarations {
                    f(ScriptDeclaration::Declarator {
                        declarator,
                        initial_dropped: true,
                    })?;
                }
            }
        };
    }

    match stmt {
        Statement::VariableDeclaration(decl) => {
            if top || (descend && decl.kind == VariableDeclarationKind::Var) {
                for declarator in decl.declarations {
                    f(ScriptDeclaration::Declarator {
                        declarator,
                        initial_dropped: porous,
                    })?;
                }
            }
        }
        Statement::FunctionDeclaration(fun) => {
            if top && let Some(id) = fun.id.as_ref() {
                f(ScriptDeclaration::Function(id))?;
            }
            // A function BODY is a new function scope — no `var` escapes it.
        }
        Statement::ClassDeclaration(class) => {
            if top && let Some(id) = class.id.as_ref() {
                f(ScriptDeclaration::Class(id))?;
            }
            // A class body is deliberately OPAQUE here — see the ⚠️ note on this
            // function. A static block's `var` genuinely does reach script scope
            // in the oracle; `script_contains_static_block` is what covers it.
        }
        Statement::ImportDeclaration(import) => {
            for spec in import.specifiers {
                let local = match spec {
                    ImportSpecifier::Default(s) => &s.local,
                    ImportSpecifier::Named(s) => &s.local,
                    ImportSpecifier::Namespace(s) => &s.local,
                };
                f(ScriptDeclaration::Import {
                    local,
                    declaration: import,
                })?;
            }
        }
        // A module `export const`/`function`/`class`/`let`/`var` binds a
        // module-scope name the evaluator must see (an `export const a = 'ok'`
        // folds a template `{a}`), so recurse into the exported declaration at
        // the SAME level. (`export { a }` / `export … from` carry no
        // `declaration` and bind no new name.)
        Statement::ExportNamedDeclaration(export) => {
            if let Some(decl) = export.declaration {
                script_declarations_of(decl, var_scope, top, porous, f)?;
            }
        }
        // `export default function name() {}` binds `name`, but an instance-script
        // export and a module `export default` each refuse on their own path
        // (`Refusal::ModuleDefaultExport` / the instance-export refusal), so no
        // consumer of this walk can reach one.
        Statement::ExportDefaultDeclaration(_) => {}
        // Statement bodies that are NOT a new function scope: a `var` declared
        // inside one is function-scoped and lands in THIS script's scope.
        Statement::BlockStatement(block) => {
            for s in block.body {
                nested_porous!(s);
            }
        }
        Statement::IfStatement(stmt) => {
            nested!(stmt.consequent);
            if let Some(alternate) = stmt.alternate {
                nested!(alternate);
            }
        }
        Statement::ForStatement(stmt) => {
            if let Some(ForInit::VariableDeclaration(decl)) = stmt.init.as_ref() {
                head_declaration!(decl);
            }
            nested_porous!(stmt.body);
        }
        Statement::ForInStatement(stmt) => {
            if let ForInOfLeft::VariableDeclaration(decl) = &stmt.left {
                head_declaration!(decl);
            }
            nested_porous!(stmt.body);
        }
        Statement::ForOfStatement(stmt) => {
            if let ForInOfLeft::VariableDeclaration(decl) = &stmt.left {
                head_declaration!(decl);
            }
            nested_porous!(stmt.body);
        }
        Statement::WhileStatement(stmt) => nested!(stmt.body),
        Statement::DoWhileStatement(stmt) => nested!(stmt.body),
        Statement::LabeledStatement(stmt) => nested!(stmt.body),
        Statement::SwitchStatement(stmt) => {
            for case in stmt.cases {
                for s in case.consequent {
                    nested_porous!(s);
                }
            }
        }
        Statement::TryStatement(stmt) => {
            for s in stmt.block.body {
                nested_porous!(s);
            }
            if let Some(handler) = stmt.handler.as_ref() {
                for s in handler.body.body {
                    nested_porous!(s);
                }
            }
            if let Some(finalizer) = stmt.finalizer.as_ref() {
                for s in finalizer.body {
                    nested_porous!(s);
                }
            }
        }
        // Declare nothing at script scope. An EXPRESSION declares nothing at all
        // except through a class expression's static block, which is the fenced
        // family (see the ⚠️ note above), so no expression position is visited.
        // (A `return` only occurs inside a function, which is a scope boundary
        // this walk never enters.)
        Statement::ExpressionStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::ReturnStatement(_)
        | Statement::BreakStatement(_)
        | Statement::ContinueStatement(_)
        | Statement::EmptyStatement(_)
        | Statement::DebuggerStatement(_)
        | Statement::ExportAllDeclaration(_) => {}
        // TypeScript-only statements. Type erasure runs before every consumer of
        // this walk, so none of these survive to reach it; the arms exist so a
        // new variant still fails compilation here.
        Statement::TSTypeAliasDeclaration(_)
        | Statement::TSInterfaceDeclaration(_)
        | Statement::TSDeclareFunction(_)
        | Statement::TSEnumDeclaration(_)
        | Statement::TSModuleDeclaration(_)
        | Statement::TSExportAssignment(_)
        | Statement::TSNamespaceExportDeclaration(_)
        | Statement::TSImportEqualsDeclaration(_) => {}
    }
    Ok(())
}

/// What a script declaration of a rune STEM (`state`, `props`, …) initializes it
/// to — the only thing the oracle's store reclassification asks about. See
/// [`refuse_rune_store_collision`].
///
/// The question is about the oracle's `binding.initial`, NOT about the source
/// text of the declarator — the two come apart for a `var` that hoisted through
/// a porous scope, whose binding carries no initializer at all.
enum StemInit {
    /// The binding's `initial` is `$props()` exactly (the oracle's
    /// `get_rune(binding.initial) === '$props'`).
    PropsRune,
    /// The binding's `initial` is some OTHER rune call (`$state(0)`,
    /// `$derived(e)`, `$state.snapshot(x)`, `$props.id()`) —
    /// `get_rune(binding.initial) !== null`.
    OtherRune,
    /// An `import { … } from 'svelte/store'` local.
    SvelteStoreImport,
    /// `get_rune(binding.initial) === null`: another import, a function/class
    /// declaration, a declarator with no init or a non-rune init, **or** a `var`
    /// whose initializer the hoist dropped.
    Plain,
}

/// Refuse a rune keyword whose `$`-stripped stem is ALSO a binding **in scope at
/// the instance script** — `import { state } from './store'` beside a `$state`
/// reference.
///
/// The oracle's `analyze_component` (`phases/2-analyze/index.js`, the "create
/// synthetic bindings for store subscriptions" loop) walks every unresolved
/// `$`-prefixed reference and, for a rune name `$stem`, reclassifies it as a
/// STORE subscription — `store_sub` binding, emitting `$.store_get(…)` — as soon
/// as `instance.scope.get(stem)` is non-null and that binding's own initializer
/// is not itself a rune-creating call. It then DELETES the reference from
/// `module.scope.references`, which is what the runes-mode inference reads a few
/// lines later, so the collision can flip the whole component out of runes mode.
///
/// ⚠️ **`instance.scope.get` walks UP the scope chain**
/// (`phases/scope.js:748` — `this.declarations.get(name) ?? this.parent?.get(name)
/// ?? null`), and the instance scope's parent is the MODULE scope
/// (`2-analyze/index.js:337` — `js(root.instance, scope_root, true, module.scope)`).
/// So a `<script module>` binding of the stem reclassifies an instance-script
/// `$stem` too, and both bodies are searched here, instance first.
///
/// It walks up only. **Downward** — a function parameter named `state`, a
/// block-scoped `let state`, a name bound in a nested FUNCTION body — is a CHILD
/// scope `instance.scope.get` never sees, so none of those collide and all keep
/// compiling. Two nested forms DO reach script scope, and they differ:
///
/// - a **`var`** anywhere below the top level except inside a function: it is
///   function-scoped, so a `var state` in any block, for-head, switch, or
///   try/catch of the script lands in `instance.scope`. But it arrives
///   **stripped of its initializer** — `scope.js:673-681` re-declares it on the
///   parent with the 4-argument `initial` left at its `null` default — so the
///   rune exemption below can never apply to one, whatever it was written with;
/// - a statement in a **class STATIC BLOCK**, which is not a scope at all:
///   `phases/scope.js` has no `StaticBlock` visitor, so a `var` there declares
///   directly in the enclosing scope and **keeps** its initializer. ECMAScript
///   says a static block is its own VariableEnvironment; the oracle is the
///   parity target, so the oracle wins. A class METHOD body is a genuine
///   boundary on both sides — the oracle's `FunctionExpression` visitor gives it
///   a scope; a class PROPERTY INITIALIZER is **not** (there is no
///   `PropertyDefinition` visitor either), so it evaluates in the enclosing scope
///   and a class expression there is as reachable as one anywhere else.
///
/// The two are handled differently, and the asymmetry is deliberate. The `var`
/// hoist is modelled EXACTLY, by [`each_script_declaration`]'s one exhaustive
/// statement enumeration, with `ScriptDeclaration::Declarator::initial_dropped`
/// carrying the initializer distinction — those shapes are ordinary real code and
/// the precision is earned. The static block is instead FENCED lexically
/// ([`script_contains_static_block`]): reaching every class body a script can
/// hold means enumerating every expression position of every statement, which is
/// the surface that shipped holes twice, and a static block in a Svelte component
/// is vanishingly rare (zero files in the ~4900-file compile corpus contain one),
/// so the precision would buy nothing and cost correctness.
///
/// tsv is a runes-only compiler and models neither the reclassification nor mode
/// inference: it would compile `$state` as the rune and silently emit the wrong
/// code (`const x = void 0` where the oracle emits a store read). Refuse instead.
///
/// The oracle's EXEMPTION covers the majority of real Svelte 5 code and is
/// modelled here: `let state = $state(0)` / `const props = $props()` keep
/// compiling, because `get_rune(binding.initial)` is non-null there. Three
/// corners of the oracle's clause come with it — a stem OTHER than `props`
/// initialized by `$props()` (`let state = $props()`) IS reclassified ("rune-line
/// names received as props are valid too"), `$derived` beside an
/// `import { derived } from 'svelte/store'` is NOT, and a rune-initialized `var`
/// that hoisted through a porous scope IS, because the initializer the exemption
/// reads was dropped on the way up (above).
///
/// It is an over-approximation in one direction: `Plain` is also what an
/// unreadable binding shape yields (an escaped identifier, a pattern the shared
/// walk declines), so a document can refuse where the oracle would have exempted.
/// A refusal is the safe side; a missed binding is a MISMATCH.
///
/// The reference test is a boundary-checked source scan rather than an AST walk:
/// tsv recognizes a rune at half a dozen scattered sites (declarator inits, the
/// statement-position `$effect`/`$inspect` drops, class fields, the rune guard's
/// sanctioned set, the template `$state.snapshot`), and a check that must be
/// wired into each of them can miss one — which is a MISMATCH. One whole-document
/// scan cannot. Its cost is over-refusing a document that merely MENTIONS
/// `$state` while also binding `state` — in a comment, in template text, in a
/// string, or as a **member/property NAME** (`obj.$state`, `{ $state: 1 }`),
/// which is not a rune reference at all. Every one is a clean refusal rather
/// than a wrong compile, which is why the scan is deliberately unbounded.
pub(crate) fn refuse_rune_store_collision<'arena>(
    instance_body: &'arena [Statement<'arena>],
    module_body: &'arena [Statement<'arena>],
    source: &str,
    interner: &SharedInterner,
) -> Result<(), CompileError> {
    let static_block = script_contains_static_block(instance_body, source)
        || script_contains_static_block(module_body, source);
    for stem in RUNE_BASES {
        let name = format!("${stem}");
        if !source_references_identifier(source, &name) {
            continue;
        }
        // The static-block fence: a class body is opaque to the declaration walk,
        // so with one present this check cannot rule out a script-scope binding of
        // the stem and refuses unconditionally.
        if static_block {
            return Err(unsupported(Refusal::RuneNameBoundAsStore { name }));
        }
        // `instance.scope.get(stem)`: the instance scope's own declarations, then
        // its parent's — the module scope.
        let Some(init) = stem_declaration(instance_body, stem, source, interner)
            .or_else(|| stem_declaration(module_body, stem, source, interner))
        else {
            continue;
        };
        let reclassified = match init {
            // `get_rune(init) === null` — the plain case.
            StemInit::Plain => true,
            StemInit::SvelteStoreImport => *stem != "derived",
            StemInit::PropsRune => *stem != "props",
            StemInit::OtherRune => false,
        };
        if reclassified {
            return Err(unsupported(Refusal::RuneNameBoundAsStore { name }));
        }
    }
    Ok(())
}

/// Whether a class **static block** occurs anywhere in `stmts`' source range.
///
/// The fence that makes the whole class-body family safe without traversing it.
/// A static block is the ONLY construct below a script's top level, other than the
/// `var` hoists [`script_declarations_of`] models exactly, that can declare a name
/// at script scope: statements appear only in function bodies (a genuine scope
/// boundary on both sides) and in static blocks (no scope at all in the oracle —
/// `phases/scope.js` has no `StaticBlock` visitor, so `class C { static { var
/// state = 5 } }` declares `state` in the ENCLOSING scope; ECMAScript disagrees,
/// but the oracle is the parity target). With those two cases covered, the
/// declaration walk can stop at every class body and every expression position.
///
/// Deliberately **lexical, not an AST walk** — that is the point. Reaching every
/// class body a statement can hold means visiting every expression position of
/// every statement, and a hand-enumerated version of that surface has twice
/// shipped with holes (a class expression in a for-head, a `super_class`, a
/// property initializer, a computed key, a parameter default…), each hole a
/// silent MISMATCH. A scan over the bytes has no positions to enumerate.
///
/// **Under-reporting is what this scan must not do**, and the whitespace class is
/// the whole of that argument. A static block is written `static`, then trivia,
/// then `{`; its `static` token always lies inside a statement's span; and the
/// trivia is JS `WhiteSpace`/`LineTerminator` or a comment. So the scan is
/// complete exactly as far as [`is_js_whitespace`] is the JS class — which it is
/// by construction, unlike Rust's `char::is_whitespace` (that one omits
/// `U+FEFF`, and a `static\u{FEFF}{ var state = 5 }` block written with it was
/// invisible here). A `/` after the trivia run may open a comment, so it counts
/// as "cannot tell" rather than decoding comment syntax.
///
/// It happily OVER-reports — `static` in a comment or a string, a `/` that turns
/// out to be a division, a `U+0085` (`<NEL>`, Unicode whitespace but NOT JS
/// whitespace, so the scan sees a boundary the JS lexer would reject anyway) —
/// and over-reporting only costs an extra refusal.
fn script_contains_static_block(stmts: &[Statement<'_>], source: &str) -> bool {
    let (Some(first), Some(last)) = (stmts.first(), stmts.last()) else {
        return false;
    };
    let range = first.span().start as usize..last.span().end as usize;
    let Some(text) = source.get(range) else {
        // An unexpected span shape is not a reason to go blind.
        return true;
    };
    let mut offset = 0;
    while let Some(found) = text[offset..].find("static") {
        let start = offset + found;
        let end = start + "static".len();
        let preceded_by_ident = text[..start]
            .chars()
            .next_back()
            .is_some_and(is_identifier_part);
        // `static` is ASCII, so `start + 1` is a char boundary.
        offset = start + 1;
        if preceded_by_ident {
            continue;
        }
        // Trivia between `static` and its `{` is JS whitespace and comments —
        // `is_js_whitespace`, NOT Rust's `char::is_whitespace`, which omits
        // `U+FEFF` and would miss a static block written with one. A `/` may open
        // a comment, so treat it as "cannot tell" rather than decoding comment
        // syntax here — the safe direction.
        if text[end..]
            .trim_start_matches(is_js_whitespace)
            .starts_with(['{', '/'])
        {
            return true;
        }
    }
    false
}

/// How `stmts` declare `stem` at script scope, or `None` when they don't (one
/// level of the oracle's `instance.scope.get(stem)` chain). A later declaration
/// wins, mirroring the scope's last-writer-wins map.
///
/// Routed through [`each_script_declaration`] — the ONE exhaustive statement
/// enumeration — so a `var` hoisted out of a nested block or for-head is seen and
/// a new `Statement` variant fails compilation rather than silently escaping the
/// guard.
fn stem_declaration<'arena>(
    stmts: &'arena [Statement<'arena>],
    stem: &str,
    source: &str,
    interner: &SharedInterner,
) -> Option<StemInit> {
    let mut found = None;
    let walk = each_script_declaration::<()>(stmts, VarScope::WithHoistedVars, &mut |decl| {
        match decl {
            ScriptDeclaration::Declarator {
                declarator,
                initial_dropped,
            } => {
                let mut names = Vec::new();
                // A pattern the shared walk can't enumerate is not a reason to
                // refuse on its own — the binding table refuses those shapes on
                // their own path — but it IS a name this check cannot rule out,
                // so it counts as declaring the stem (a safe over-refusal). Same
                // for an escaped binding identifier, which
                // `pattern_binding_names` skips outright.
                let unnameable = pattern_binding_names(&declarator.id, source, &mut names).is_err()
                    || pattern_binds_unnameable_identifier(&declarator.id);
                if !unnameable && !names.iter().any(|n| n == stem) {
                    return Ok(());
                }
                // A `var` that hoisted through a porous scope arrives with NO
                // initializer (`ScriptDeclaration::Declarator::initial_dropped`),
                // so the oracle's `get_rune(binding.initial)` sees `null` and the
                // rune EXEMPTION does not apply however the declarator's own init
                // reads.
                found = Some(match declarator.init.as_ref() {
                    Some(init) if !initial_dropped => match classify_rune_init(init, source) {
                        Some(RuneInit::Props) => StemInit::PropsRune,
                        Some(_) => StemInit::OtherRune,
                        None => StemInit::Plain,
                    },
                    _ => StemInit::Plain,
                });
            }
            ScriptDeclaration::Function(id) | ScriptDeclaration::Class(id) => {
                if identifier_name(id, source, interner) == stem {
                    found = Some(StemInit::Plain);
                }
            }
            ScriptDeclaration::Import { local, declaration } => {
                if identifier_name(local, source, interner) == stem {
                    let from_store = matches!(
                        &declaration.source.value,
                        LiteralValue::String(s) if s.resolve(declaration.source.span, source) == "svelte/store"
                    );
                    found = Some(if from_store {
                        StemInit::SvelteStoreImport
                    } else {
                        StemInit::Plain
                    });
                }
            }
        }
        Ok(())
    });
    // The callback never fails.
    debug_assert!(walk.is_ok());
    found
}

/// An identifier's name, escaped forms included (`state` → `state`).
///
/// Unlike [`plain_identifier_name`] this never returns `None`: a binding whose
/// name this check cannot read is a binding it would MISS, and a missed binding
/// is an under-refusal.
fn identifier_name(
    id: &tsv_ts::ast::internal::Identifier<'_>,
    source: &str,
    interner: &SharedInterner,
) -> String {
    id.name(source, &interner.borrow()).to_string()
}

/// Whether `name` (a `$`-prefixed rune keyword) occurs in `source` as a whole
/// identifier — bounded on both sides by a character that is not an ECMAScript
/// `IdentifierPart`. Deliberately blind to comments, strings, and template text:
/// see [`refuse_rune_store_collision`] for why over-detection there is the safe
/// direction.
///
/// The boundary test decodes CHARACTERS, not bytes. A byte-level "every byte
/// `>= 0x80` continues an identifier" shortcut reads the lead byte of a non-ASCII
/// **whitespace** character — NBSP (U+00A0, `0xC2 0xA0`) is ECMAScript
/// whitespace — as identifier text, so `$state (1)` written with an NBSP would
/// not match and a genuine reference would be MISSED. That is an under-refusal,
/// the direction this whole check exists to avoid.
fn source_references_identifier(source: &str, name: &str) -> bool {
    // The `start + 1` resume below is a char boundary because every caller passes
    // a `$`-prefixed rune keyword, so byte 0 of a match is the ASCII `$`.
    debug_assert!(name.starts_with('$'));
    let mut offset = 0;
    while let Some(found) = source[offset..].find(name) {
        let start = offset + found;
        let end = start + name.len();
        let before_ok = !source[..start]
            .chars()
            .next_back()
            .is_some_and(is_identifier_part);
        let after_ok = !source[end..].chars().next().is_some_and(is_identifier_part);
        if before_ok && after_ok {
            return true;
        }
        offset = start + 1;
    }
    false
}

/// ECMAScript `IdentifierPart`, for the boundary test above.
///
/// `XID_Continue` plus `$` (`_` and ZWNJ/ZWJ are already in `XID_Continue`).
/// ECMAScript actually uses the slightly wider `ID_Continue`; the handful of code
/// points in `ID_Continue \ XID_Continue` therefore read as a BOUNDARY here,
/// which makes an adjacent `$state\u{309B}` match as a whole identifier — an
/// over-refusal, the safe direction.
fn is_identifier_part(ch: char) -> bool {
    unicode_ident::is_xid_continue(ch) || ch == '$'
}

pub(crate) fn plain_identifier_name(
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
pub(crate) fn refuse_runes_invalid_import(
    import: &ImportDeclaration<'_>,
    source: &str,
) -> Result<(), CompileError> {
    // Checked here rather than in the guard walk because the transform hoists
    // imports out of the statement stream before `walk_statement` runs. The rule
    // — including the type-only-import caveat — lives at
    // `refuse_dollar_import_locals`.
    refuse_dollar_import_locals(import.specifiers, source)?;
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
    seen_props: &mut bool,
) -> Result<(), CompileError> {
    let rune = declarator
        .init
        .as_ref()
        .and_then(|init| classify_rune_init(init, source));

    match rune {
        Some(RuneInit::Props) => {
            // The oracle rejects a second `$props()` (`props_duplicate`) from its
            // analyze-phase `CallExpression` visitor
            // (`phases/2-analyze/visitors/CallExpression.js:68-73`), BEFORE the
            // placement check — so the duplicate wins over `props_invalid_placement`
            // when both apply. Only a top-level declarator init is inspected here;
            // a `$props()` in any other position already refuses on its own path,
            // so this sees every shape tsv would otherwise accept.
            if *seen_props {
                return Err(unsupported(Refusal::DuplicateProps));
            }
            *seen_props = true;
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
        Some(RuneInit::PropsId) => {
            // `const id = $props.id()` binds a plain identifier only (the oracle's
            // `props_id_invalid_placement` rejects a destructure). The binding
            // evaluates through the `$props.id()` call — the evaluator maps that
            // keypath to a STRING sentinel, so a `{id}` read never folds (matching
            // the oracle's `$.escape(id)`).
            let name = identifier_binding_name(&declarator.id, source)
                .ok_or_else(|| unsupported(Refusal::PropsIdBindingPattern))?;
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
        Some(RuneInit::StateSnapshot(_)) => {
            // `const s = $state.snapshot(x)` unwraps to `const s = x` for EMISSION,
            // but the binding stays UNKNOWN to the evaluator — the unwrap is the
            // emission form, not the evaluation form. The oracle evaluates a rune
            // declarator through its argument for `$state` / `$state.raw` /
            // `$derived` only; every other rune, `$state.snapshot` included, falls
            // to the `default` arm and yields UNKNOWN, so a `{s}` read never folds
            // (`$.escape(s)`). That holds however the argument itself evaluates —
            // a plain `let` argument does not fold either.
            //
            // A destructured target refuses — the oracle lowers
            // `const {a} = $state.snapshot(x)` into a temp-destructure
            // (`const tmp = x, a = tmp.a`), a shape this transform does not
            // reproduce (a safe over-refusal).
            let name = identifier_binding_name(&declarator.id, source)
                .ok_or_else(|| unsupported(Refusal::DestructuringStateSnapshot))?;
            bindings.insert(
                name,
                Binding {
                    kind: BindingKind::Normal,
                    initial: Initial::None,
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

pub(crate) fn identifier_binding_name(id: &Expression<'_>, source: &str) -> Option<String> {
    let Expression::Identifier(ident) = id else {
        return None;
    };
    plain_identifier_name(ident, source)
}

/// Rewrite one instance-script statement for the server module:
///
/// - a top-level `$props()` declarator init becomes `$$props` (and the
///   component gains the `$$props` param); a `$bindable(fallback?)` default in
///   the destructure pattern is rewritten to its fallback (`void 0` when
///   argument-less) and the prop is collected into `bindable` so the transform
///   appends the trailing `$.bind_props($$props, { … })`;
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
pub(crate) fn rewrite_script_statement<'arena>(
    b: &mut Builder<'arena>,
    stmt: &'arena Statement<'arena>,
    source: &str,
    derived_names: &NameSet,
    store_names: &NameSet,
    updated: &mut NameSet,
    nested_declared: &mut NameSet,
    uses_props: &mut bool,
    has_effects: &mut bool,
    has_comments: bool,
    uses_slots: bool,
    dropped_regions: &mut Vec<Span>,
    bindable: &mut Vec<BindableEntry>,
    props_id: &mut Option<String>,
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
        let mut ctx = WalkCtx::new(
            source,
            updated,
            nested_declared,
            derived_names,
            std::rc::Rc::clone(&b.interner),
        )
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal
        // and `$derived` reads from the derived-read refusal: the store rewrite
        // turns both into `$.store_get(…)` / `d()` after the loop. Both shadow
        // refusals are deferred (the store's needs the full nested-scope set, so
        // pass `None`; the derived's is a whole-compile check in `compile_server`).
        .allow_store_reads(store_names, None)
        .allow_derived_reads();
        walk_expression_guarded(callback, &mut ctx)?;
        return Ok(None);
    }

    // Statement-position `$inspect(…)` (bare or `.with(cb)`) is dropped on the
    // server, like `$effect` — but it does NOT force the wrapper on its own.
    // The `.with` / prop-rooted-argument cases that DO wrap are already covered
    // by `needs_context` (which walks the raw instance body — `$inspect`
    // statements included — before this drop). The arguments and `.with`
    // callback are still guard-walked so a stray rune (`$inspect($state(x))`,
    // which the oracle rejects) or a derived read refuses; the `$inspect` callee
    // itself is exempt at this recognized position.
    if let Statement::ExpressionStatement(expr_stmt) = stmt
        && let Some(guarded) = is_inspect_call(&expr_stmt.expression, source)
    {
        dropped_regions.push(stmt.span());
        let mut ctx = WalkCtx::new(
            source,
            updated,
            nested_declared,
            derived_names,
            std::rc::Rc::clone(&b.interner),
        )
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal
        // and `$derived` reads from the derived-read refusal: the store rewrite
        // turns both into `$.store_get(…)` / `d()` after the loop. Both shadow
        // refusals are deferred (the store's needs the full nested-scope set, so
        // pass `None`; the derived's is a whole-compile check in `compile_server`).
        .allow_store_reads(store_names, None)
        .allow_derived_reads();
        for expr in guarded {
            walk_expression_guarded(expr, &mut ctx)?;
        }
        return Ok(None);
    }

    // A top-level class declaration may carry `$state`/`$state.raw` fields, which
    // the server unwraps exactly like a top-level `$state` declarator. Every other
    // member — a `$derived` field, a static/computed rune field, a method body, a
    // nested class — takes the normal refusing guard walk, so the guard-exempt set
    // equals the unwrap set: reach-matched by construction (see
    // `rewrite_class_state_fields`).
    if let Statement::ClassDeclaration(class) = stmt {
        return rewrite_class_state_fields(
            b,
            class,
            source,
            derived_names,
            store_names,
            updated,
            nested_declared,
            dropped_regions,
        )
        .map(Some);
    }

    let Statement::VariableDeclaration(decl) = stmt else {
        let mut ctx = WalkCtx::new(
            source,
            updated,
            nested_declared,
            derived_names,
            std::rc::Rc::clone(&b.interner),
        )
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal
        // and `$derived` reads from the derived-read refusal: the store rewrite
        // turns both into `$.store_get(…)` / `d()` after the loop. Both shadow
        // refusals are deferred (the store's needs the full nested-scope set, so
        // pass `None`; the derived's is a whole-compile check in `compile_server`).
        .allow_store_reads(store_names, None)
        .allow_derived_reads();
        walk_statement_guarded(stmt, &mut ctx, 0)?;
        return Ok(Some(stmt.clone()));
    };

    let has_rune_init = decl.declarations.iter().any(|d| {
        d.init
            .as_ref()
            .is_some_and(|i| classify_rune_init(i, source).is_some())
    });
    if !has_rune_init {
        let mut ctx = WalkCtx::new(
            source,
            updated,
            nested_declared,
            derived_names,
            std::rc::Rc::clone(&b.interner),
        )
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal
        // and `$derived` reads from the derived-read refusal: the store rewrite
        // turns both into `$.store_get(…)` / `d()` after the loop. Both shadow
        // refusals are deferred (the store's needs the full nested-scope set, so
        // pass `None`; the derived's is a whole-compile check in `compile_server`).
        .allow_store_reads(store_names, None)
        .allow_derived_reads();
        walk_statement_guarded(stmt, &mut ctx, 0)?;
        return Ok(Some(stmt.clone()));
    }

    let mut declarations: BumpVec<'arena, VariableDeclarator<'arena>> = BumpVec::new_in(b.arena);
    for declarator in decl.declarations {
        let mut ctx = WalkCtx::new(
            source,
            updated,
            nested_declared,
            derived_names,
            std::rc::Rc::clone(&b.interner),
        )
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal
        // and `$derived` reads from the derived-read refusal: the store rewrite
        // turns both into `$.store_get(…)` / `d()` after the loop. Both shadow
        // refusals are deferred (the store's needs the full nested-scope set, so
        // pass `None`; the derived's is a whole-compile check in `compile_server`).
        .allow_store_reads(store_names, None)
        .allow_derived_reads();
        // The `$`-prefixed BINDING rule, at the one point every declarator on
        // this path passes — before the rune dispatch below, mirroring the
        // oracle's own `VariableDeclarator` visitor, which runs
        // `validate_identifier_name` over every `extract_paths` leaf ahead of
        // its rune branch (`2-analyze/visitors/VariableDeclarator.js:24-26`).
        // It cannot ride the guard walk: none of the three arms below reaches
        // the binding leaves with the rule applied — a rune declarator's id is
        // not walked at all, and the two that are walked go through
        // `walk_expression_guarded`, which sees a pattern as an expression and
        // takes the store-read exemption this `WalkCtx` enables.
        refuse_dollar_binding_pattern(&declarator.id, source)?;
        let rune = declarator
            .init
            .as_ref()
            .and_then(|init| classify_rune_init(init, source));

        // `$props.id()` — skip the declarator entirely: the transform hoists
        // `const <name> = $.props_id($$renderer)` to the top of the component body
        // (the oracle's `component_block.body.unshift`, for hydration). At most one
        // per component (`props_duplicate`), and a plain-identifier target only
        // (`props_id_invalid_placement` rejects a destructure). The whole declarator
        // is a dropped region, so a comment inside refuses.
        if matches!(rune, Some(RuneInit::PropsId)) {
            if props_id.is_some() {
                return Err(unsupported(Refusal::DuplicatePropsId));
            }
            let name = identifier_binding_name(&declarator.id, source)
                .ok_or_else(|| unsupported(Refusal::PropsIdBindingPattern))?;
            *props_id = Some(name);
            dropped_regions.push(declarator.span);
            continue;
        }

        // Guard the binding pattern (a rune or derived read can hide in a
        // pattern default) — except for state/derived declarators, whose id is
        // an enforced plain identifier and is a *declaration* of the (possibly
        // derived) name, not a read. A `$props()` pattern is guard-walked AFTER
        // its bindable rewrite (in the arm below), so an exempt `$bindable(...)`
        // default — rewritten to its fallback — isn't seen as a stray rune, while
        // a `$bindable` left in any UNrecognized position survives the rewrite and
        // still refuses.
        if rune.is_none() {
            walk_expression_guarded(&declarator.id, &mut ctx)?;
        }
        // A rune init rewrite drops the call's own syntax around the kept
        // argument — record the dropped region(s) so comments inside refuse.
        if let (Some(init), Some(_)) = (&declarator.init, &rune) {
            let init_span = init.span();
            match rune {
                // `$state(v)` / `$state.snapshot(x)` unwrap to the bare argument (no
                // synthesized syntax around it), so the borrowed argument carries its
                // own interior comments and only the call syntax around it is dropped.
                Some(RuneInit::State(Some(arg))) | Some(RuneInit::StateSnapshot(arg)) => {
                    let arg_span = arg.span();
                    dropped_regions.push(Span::new(init_span.start, arg_span.start));
                    dropped_regions.push(Span::new(arg_span.end, init_span.end));
                }
                // `$derived(e)` / `$derived.by(f)` wrap the argument in a synthesized
                // `() => …` arrow whose param-list span sweeps a comment INTERIOR to the
                // argument into a double-print (and the oracle relocates it). Drop the
                // WHOLE init span so a comment anywhere inside refuses — the argument's
                // borrowed expression must not carry a comment through the arrow synthesis.
                _ => dropped_regions.push(init_span),
            }
        }

        let mut new_id = declarator.id.clone();
        // `RuneInit::PropsId` is skipped via `continue` above, so the arm below is
        // genuinely dead — it documents that invariant rather than a live branch.
        #[allow(clippy::unreachable)]
        let new_init = match rune {
            Some(RuneInit::Props) => {
                *uses_props = true;
                let (rewritten, entries) =
                    rewrite_props_pattern(b, &declarator.id, source, has_comments, uses_slots)?;
                if let Some(rewritten) = rewritten {
                    new_id = rewritten;
                }
                bindable.extend(entries);
                // Guard-walk the REWRITTEN pattern: the recognized top-level
                // `$bindable(...)` defaults are now their fallback expressions, so
                // a stray rune / derived read inside a fallback still refuses,
                // while a `$bindable` in any unrecognized position (nested, wrong
                // arity, non-identifier key/local) survived the rewrite and
                // refuses here.
                walk_expression_guarded(&new_id, &mut ctx)?;
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
            // Handled above by `continue` — the declarator is skipped, never
            // rebuilt, so this arm is unreachable. Kept for match exhaustiveness.
            Some(RuneInit::PropsId) => unreachable!("$props.id() is skipped above"),
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
            // `$state.snapshot(x)` unwraps to `x` (like `$state`), guarding `x`.
            Some(RuneInit::StateSnapshot(arg)) => {
                walk_expression_guarded(arg, &mut ctx)?;
                Some(arg.clone())
            }
            Some(RuneInit::Derived(expr)) => {
                // `$derived(d)` whose WHOLE body is a bare `$derived` read: the
                // oracle rewrites the read to `d()`, then `unthunk` collapses
                // `() => d()` to `d`, emitting `$.derived(d)` — the derived
                // function passed directly, never read. The script rewrite can't
                // reproduce that collapse (its store-rewrite pass would turn the
                // argument into `d()`, giving `$.derived(() => d())`), so refuse —
                // a safe over-refusal (the read refused before this slice too).
                refuse_bare_derived_arg(expr, source, derived_names)?;
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
                //
                // The synthetic `$.derived(...)` and its arrow steal the replaced
                // `$derived(...)` init's host span so a carried script comment's
                // declarator/call windows stay empty (`derived_call`), the
                // call-structure analog of the `$$props` span-steal above.
                let anchor = declarator
                    .init
                    .as_ref()
                    .map_or(declarator.span, Expression::span);
                let argument = match unthunk_callee(expr) {
                    Some(callee) => callee,
                    None => &*b.arena.alloc(b.arrow_expr_at(anchor, expr)),
                };
                Some(b.derived_call(anchor, argument))
            }
            Some(RuneInit::DerivedBy(f)) => {
                // `$derived.by(d)` passes `d` straight through as the compute
                // function → `$.derived(d)` (`.by` runs no `unthunk`), and the
                // store-rewrite pass then lowers the bare `d` read to `d()` →
                // `$.derived(d())`, exactly the oracle's output — so no refusal is
                // needed (unlike the `$derived(d)` arm, whose `() => d()` the oracle
                // collapses to `$.derived(d)`, a form the rewrite can't reproduce).
                walk_expression_guarded(f, &mut ctx)?;
                let anchor = declarator
                    .init
                    .as_ref()
                    .map_or(declarator.span, Expression::span);
                Some(b.derived_call(anchor, f))
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
    // Every declarator was a skipped `$props.id()` — drop the whole statement
    // (its `id` binding lives on as the hoisted `const id = $.props_id($$renderer)`).
    if declarations.is_empty() {
        return Ok(None);
    }
    Ok(Some(Statement::VariableDeclaration(VariableDeclaration {
        kind: decl.kind,
        declarations: declarations.into_bump_slice(),
        declare: decl.declare,
        span: decl.span,
    })))
}

/// Rewrite a top-level class declaration for the server module: unwrap each
/// **direct** `$state(v)` / `$state.raw(v)` class field to its argument (exactly
/// like a top-level `$state` declarator init), and guard-walk every other member
/// through the normal refusing path.
///
/// The unwrap set is deliberately narrow — a non-static, non-computed field whose
/// init [`classify_rune_init`] recognizes as [`RuneInit::State`] — and it EXACTLY
/// equals the set the guard exempts, because every member that is not that shape
/// (a `$derived` field, a `static`/computed rune field, a method body, a nested
/// class or class expression inside one) flows through
/// [`walk_class_member_guarded`], the same refusing walk a class in any other
/// position takes. So a member is exempted from refusal iff it is unwrapped here:
/// there is no reach gap where the guard would pass a `$state` field the transform
/// leaves referencing an undefined `$state` (a MISMATCH). The reach is structural
/// — only a top-level `Statement::ClassDeclaration` reaches this function.
///
/// Oracle shape: `field = $state(v)` → `field = v`; a no-arg `field = $state()` →
/// a BARE field `field;` (the value dropped, NOT `void 0` — the divergence from the
/// top-level no-arg declarator, which mints `void 0`); a `static`/computed field is
/// oracle-rejected placement and refuses here. Non-rune members clone through in
/// source order (the class member order is preserved). Only the call syntax around
/// the kept argument is dropped, recorded in `dropped_regions` so a comment inside
/// refuses.
///
/// The member list is rebuilt **lazily** (the `erase.rs::class_body`
/// structural-sharing idiom): `out` stays `None` — allocating nothing — until the
/// first `$state` field is unwrapped, at which point the untouched prefix is
/// backfilled once; a rune-free top-level class (the common case) returns
/// `class.clone()` having allocated no member `Vec`. The per-member side effects —
/// the guard walk, the refusal checks, the `dropped_regions` pushes — run for every
/// member regardless of whether `out` ever materializes.
#[allow(clippy::too_many_arguments)]
fn rewrite_class_state_fields<'arena>(
    b: &Builder<'arena>,
    class: &'arena ClassDeclaration<'arena>,
    source: &str,
    derived_names: &NameSet,
    store_names: &NameSet,
    updated: &mut NameSet,
    nested_declared: &mut NameSet,
    dropped_regions: &mut Vec<Span>,
) -> Result<Statement<'arena>, CompileError> {
    let arena = b.arena;
    // This path intercepts the statement before the guard walk's
    // `ClassDeclaration` arm, so it owns the class id's binding check.
    if let Some(id) = &class.id {
        refuse_dollar_binding_name(id, source)?;
    }
    let mut ctx = WalkCtx::new(
        source,
        updated,
        nested_declared,
        derived_names,
        std::rc::Rc::clone(&b.interner),
    )
    // Same exemptions as the surrounding script guard: a `$name` store read / a
    // `$derived` read inside a method body is rewritten later, not refused here.
    .allow_store_reads(store_names, None)
    .allow_derived_reads();

    let members = class.body.body;
    let mut out: Option<BumpVec<'arena, ClassMember<'arena>>> = None;
    for (i, member) in members.iter().enumerate() {
        // The per-member step produces `Some(replacement)` for an unwrapped
        // `$state` field and `None` for an unchanged member — AFTER running the
        // member's side effects. The lazy `out` then decides allocation only.
        let replacement = if let ClassMember::PropertyDefinition(p) = member
            // The one exempt shape — a DIRECT top-level `$state`/`$state.raw`
            // field. `!is_static && !computed` keeps static/computed rune fields
            // (which the oracle rejects as `state_invalid_placement`) on the
            // refusing path.
            && !p.is_static
            && !p.computed
            && let Some(value) = &p.value
            && let Some(RuneInit::State(arg)) = classify_rune_init(value, source)
        {
            let init_span = value.span();
            let new_value = match arg {
                // `field = $state(v)` → `field = v`: guard-walk the borrowed
                // argument, drop the call syntax around it.
                Some(arg) => {
                    // A LONE reactive-binding argument (`$state($count)` /
                    // `$state(d)`) refuses: the oracle keeps such a lone store /
                    // `$derived` read BARE in the unwrapped field, but tsv's store
                    // rewrite descends into class bodies and would rewrite the kept
                    // argument to `$.store_get(…)` / `d()` — a MISMATCH. A compound
                    // (`$state($count + 1)`) or a plain-variable argument is fine —
                    // the inner read there IS rewritten at parity.
                    if is_lone_reactive_binding(arg, source, derived_names, store_names) {
                        return Err(unsupported(Refusal::ClassFieldStateReactiveArg));
                    }
                    walk_expression_guarded(arg, &mut ctx)?;
                    let arg_span = arg.span();
                    dropped_regions.push(Span::new(init_span.start, arg_span.start));
                    dropped_regions.push(Span::new(arg_span.end, init_span.end));
                    Some(arg.clone())
                }
                // `field = $state()` → a bare field `field;` (value dropped, no
                // `void 0`). The whole call is a dropped region.
                None => {
                    dropped_regions.push(init_span);
                    None
                }
            };
            Some(ClassMember::PropertyDefinition(PropertyDefinition {
                value: new_value,
                ..p.clone()
            }))
        } else {
            // Every other member — the normal refusing guard walk.
            walk_class_member_guarded(member, &mut ctx)?;
            None
        };

        match replacement {
            // Unchanged — only clone into `out` once it has been materialized.
            None => {
                if let Some(vec) = out.as_mut() {
                    vec.push(member.clone());
                }
            }
            // Changed — materialize `out` (backfilling the untouched prefix
            // `members[..i]` on the first change) and push the replacement.
            Some(new) => out
                .get_or_insert_with(|| {
                    let mut vec = BumpVec::with_capacity_in(members.len(), arena);
                    vec.extend_from_slice(&members[..i]);
                    vec
                })
                .push(new),
        }
    }

    match out {
        // No `$state` field — allocated nothing; clone the whole class through.
        None => Ok(Statement::ClassDeclaration(class.clone())),
        Some(members) => Ok(Statement::ClassDeclaration(ClassDeclaration {
            body: ClassBody {
                body: members.into_bump_slice(),
                span: class.body.span,
            },
            ..class.clone()
        })),
    }
}

/// Whether `arg` — the WHOLE argument of a class-field `$state(…)` /
/// `$state.raw(…)` — is a lone reactive-binding identifier the store rewrite
/// would otherwise rewrite: a **store read** (a plain `$name` whose `$`-stripped
/// base is a store binding and not a rune) or a **`$derived` binding** read.
///
/// Mirrors `store_rewrite`'s `store_base` / `derived_read` decision (both skip
/// escaped identifiers via `plain_identifier_name` — so an escaped lone argument
/// is not caught here, matching the store rewrite, which would not rewrite it
/// either; an escaped derived read is separately refused by the guard). The
/// discriminant is exactly "would the store rewrite touch this lone identifier?",
/// so the refusal covers precisely the shapes the oracle keeps bare and nothing
/// wider — a compound argument (`$state($count + 1)` → `$.store_get(…) + 1`) or a
/// plain-variable argument stays compiling.
fn is_lone_reactive_binding(
    arg: &Expression<'_>,
    source: &str,
    derived_names: &NameSet,
    store_names: &NameSet,
) -> bool {
    let Expression::Identifier(id) = arg else {
        return false;
    };
    let Some(name) = plain_identifier_name(id, source) else {
        return false;
    };
    if derived_names.contains(&name) {
        return true;
    }
    crate::analyze::store_read_base(&name).is_some_and(|base| store_names.contains(base))
}

/// A bindable prop the transform must list in the trailing
/// `$.bind_props($$props, { … })` (source order). `key` is the prop name as
/// declared in the `$props()` object pattern; `local` is the destructure value —
/// they differ for a renamed prop (`{ value: v = $bindable() }` → `value`/`v`).
pub(crate) struct BindableEntry {
    /// The prop key as declared in the `$props()` object pattern.
    pub key: String,
    /// The local binding name (the destructured value).
    pub local: String,
}

/// The fallback of a rewritable `$bindable(...)` destructure default.
enum BindableDefault<'arena> {
    /// `$bindable()` — argument-less; the default becomes `void 0`.
    ArgLess,
    /// `$bindable(fallback)` — the default becomes `fallback`.
    Arg(&'arena Expression<'arena>),
}

/// Classify a destructure default's right side as a rewritable `$bindable(...)`
/// call (a plain `$bindable` callee with zero or one argument). `None` for
/// anything else — a non-call, a member callee, or `$bindable(a, b)` (the oracle
/// rejects that arity, `rune_invalid_arguments_length`) — so the `$bindable` call
/// survives the rewrite and the guard walk refuses it.
fn bindable_default<'arena>(
    right: &'arena Expression<'arena>,
    source: &str,
) -> Option<BindableDefault<'arena>> {
    let Expression::CallExpression(call) = right else {
        return None;
    };
    let Expression::Identifier(callee) = call.callee else {
        return None;
    };
    if plain_identifier_name(callee, source).as_deref() != Some("$bindable") {
        return None;
    }
    match call.arguments {
        [] => Some(BindableDefault::ArgLess),
        [arg] => Some(BindableDefault::Arg(arg)),
        _ => None,
    }
}

/// If this object-pattern property is a top-level `key = $bindable(fallback?)`
/// default the transform can rewrite — a plain-identifier key, a plain-identifier
/// destructure value, and a rewritable `$bindable(...)` right — return the entry,
/// the `Property`, the `AssignmentPattern`, and the fallback argument. `None`
/// otherwise: the property is emitted unchanged, and a `$bindable` in any
/// unrecognized shape (a non-identifier key — string/numeric/computed —, a
/// nested-pattern value, the wrong arity) survives the rewrite for the guard to
/// refuse — a safe over-refusal, even for a non-identifier-keyed prop the oracle
/// would compile.
#[allow(clippy::type_complexity)]
fn bindable_property<'arena>(
    prop: &'arena ObjectPatternProperty<'arena>,
    source: &str,
) -> Option<(
    BindableEntry,
    &'arena Property<'arena>,
    &'arena AssignmentPattern<'arena>,
    BindableDefault<'arena>,
)> {
    let ObjectPatternProperty::Property(p) = prop else {
        return None;
    };
    if p.computed {
        return None;
    }
    let Expression::AssignmentPattern(assign) = &p.value else {
        return None;
    };
    let default = bindable_default(assign.right, source)?;
    let Expression::Identifier(key_id) = &p.key else {
        return None;
    };
    let key = plain_identifier_name(key_id, source)?;
    let Expression::Identifier(left_id) = assign.left else {
        return None;
    };
    let local = plain_identifier_name(left_id, source)?;
    Some((BindableEntry { key, local }, p, assign, default))
}

/// Rebuild an object-pattern property, replacing its `$bindable(fallback?)`
/// default with the fallback (`void 0` when argument-less). A shallow re-slot:
/// the key, the `AssignmentPattern.left`, and every flag stay borrowed; only the
/// default's `right` changes.
fn rewrite_bindable_default<'arena>(
    b: &mut Builder<'arena>,
    p: &'arena Property<'arena>,
    assign: &'arena AssignmentPattern<'arena>,
    default: BindableDefault<'arena>,
) -> ObjectPatternProperty<'arena> {
    let new_right: &'arena Expression<'arena> = match default {
        BindableDefault::Arg(arg) => arg,
        BindableDefault::ArgLess => b.arena.alloc(b.void_zero()),
    };
    let new_value = Expression::AssignmentPattern(AssignmentPattern {
        left: assign.left,
        right: new_right,
        decorators: assign.decorators,
        span: assign.span,
    });
    ObjectPatternProperty::Property(Property {
        key: p.key.clone(),
        value: new_value,
        kind: p.kind,
        shorthand: p.shorthand,
        computed: p.computed,
        method: p.method,
        span: p.span,
    })
}

/// Rewrite a `$props()` binding pattern for the server module: replace each
/// recognized top-level `$bindable(fallback?)` default with its fallback
/// (collecting the bindable props in source order), and inject `$$slots,
/// $$events` wherever a rest element captures the remaining props (probe-verified):
///
/// - `let {a, ...rest} = $props()` →
///   `let { a, $$slots, $$events, ...rest } = $$props;` — injected immediately
///   BEFORE the rest element;
/// - `let props = $props()` (non-destructured) →
///   `let { $$slots, $$events, ...props } = $$props;`;
/// - `let { value = $bindable(42) } = $props()` → `let { value = 42 } = $$props;`
///   plus a `value` entry;
/// - a plain destructure with neither a rest nor a bindable default gets NO
///   rewrite.
///
/// Returns `(replacement pattern, bindable entries)`. The replacement is `None`
/// when nothing changed, so the original borrowed pattern is kept. Refuses a
/// non-identifier/non-object `$props()` pattern (the oracle rejects those —
/// props_invalid_identifier) and both rewrites alongside carried comments (the
/// minted appendix spans between host-span siblings would sweep host comments — a
/// safe over-refusal).
///
/// When the component references `$$slots` (`uses_slots`), the injected
/// sanitize_slots const owns that name, so the destructured prop deconflicts by
/// renaming: `$$slots: $$slots_` (the oracle's `VariableDeclaration.js:56-73`
/// rule — always the `_` suffix, unconditional; `$$events` never renames, and a
/// user `$$slots_`/`$$events` reference or declaration is oracle-rejected input,
/// so no second-order collision exists).
fn rewrite_props_pattern<'arena>(
    b: &mut Builder<'arena>,
    id: &'arena Expression<'arena>,
    source: &str,
    has_comments: bool,
    uses_slots: bool,
) -> Result<(Option<Expression<'arena>>, Vec<BindableEntry>), CompileError> {
    let arena = b.arena;
    match id {
        Expression::ObjectPattern(obj) => {
            let has_rest = obj
                .properties
                .iter()
                .any(|p| matches!(p, ObjectPatternProperty::RestElement(_)));
            let has_bindable = obj
                .properties
                .iter()
                .any(|p| bindable_property(p, source).is_some());
            if !has_rest && !has_bindable {
                return Ok((None, Vec::new()));
            }
            if has_comments {
                return Err(unsupported(if has_bindable {
                    Refusal::CommentsWithBindable
                } else {
                    Refusal::CommentsWithRestProps
                }));
            }
            let mut entries = Vec::new();
            let mut properties: BumpVec<'arena, ObjectPatternProperty<'arena>> =
                BumpVec::new_in(arena);
            for prop in obj.properties {
                if matches!(prop, ObjectPatternProperty::RestElement(_)) {
                    properties.push(slots_pattern_prop(b, uses_slots));
                    properties.push(shorthand_pattern_prop(b, "$$events"));
                    properties.push(prop.clone());
                } else if let Some((entry, p, assign, default)) = bindable_property(prop, source) {
                    entries.push(entry);
                    properties.push(rewrite_bindable_default(b, p, assign, default));
                } else {
                    properties.push(prop.clone());
                }
            }
            Ok((
                Some(Expression::ObjectPattern(ObjectPattern {
                    properties: properties.into_bump_slice(),
                    optional: obj.optional,
                    type_annotation: obj.type_annotation.clone(),
                    decorators: obj.decorators,
                    span: obj.span,
                })),
                entries,
            ))
        }
        Expression::Identifier(_) => {
            if has_comments {
                return Err(unsupported(Refusal::CommentsWithNonDestructuredProps));
            }
            let mut properties: BumpVec<'arena, ObjectPatternProperty<'arena>> =
                BumpVec::new_in(arena);
            properties.push(slots_pattern_prop(b, uses_slots));
            properties.push(shorthand_pattern_prop(b, "$$events"));
            properties.push(ObjectPatternProperty::RestElement(RestElement {
                argument: arena.alloc(id.clone()),
                optional: false,
                type_annotation: None,
                span: id.span(),
            }));
            Ok((
                Some(Expression::ObjectPattern(ObjectPattern {
                    properties: properties.into_bump_slice(),
                    optional: false,
                    type_annotation: None,
                    decorators: None,
                    span: id.span(),
                })),
                Vec::new(),
            ))
        }
        _ => Err(unsupported(Refusal::PropsBindingPattern)),
    }
}

/// The injected `$$slots` pattern property: shorthand `{ $$slots }` normally,
/// renamed `{ $$slots: $$slots_ }` when the sanitize_slots const owns the name
/// (see `rewrite_props_pattern`).
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
