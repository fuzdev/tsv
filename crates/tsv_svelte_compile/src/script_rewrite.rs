//! Instance-script analysis and rewrite for the server transform.
//!
//! The document-wide TypeScript flag and gate, the top-level binding-table
//! analysis, the per-statement rune rewrites (`$props()` → `$$props`,
//! `$state`/`$derived` unwrap, dropped $effect), and the erase self-check that
//! closes the loop on the finished program. See [`crate::transform_server`] for
//! the orchestration that calls these in sequence.

use bumpalo::collections::Vec as BumpVec;
use tsv_lang::{InfallibleResolve, Span};
use tsv_svelte::ast::internal::{AttributeNode, AttributeValue, Root};
use tsv_ts::ast::internal::{
    AssignmentPattern, Expression, ImportDeclaration, ImportSpecifier, LiteralValue,
    ModuleExportName, ObjectPattern, ObjectPatternProperty, Property, PropertyKind, RestElement,
    Statement, VariableDeclaration, VariableDeclarator,
};

use crate::analyze::{
    Binding, BindingKind, Bindings, Initial, NameSet, RuneInit, classify_rune_init, is_effect_call,
    is_inspect_call, pattern_binding_names,
};
use crate::attr_refs::{TemplateItem, each_template_item};
use crate::build::Builder;
use crate::rune_guard::{WalkCtx, walk_expression_guarded, walk_statement_guarded};
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
pub(crate) fn collect_script_comments(
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
    // A carried comment is placed as a leading comment of a *surviving body*
    // statement — one that prints in the component function from source. An
    // `import` hoists to a separate module-scope program (comment-free), and a
    // statement-position `$effect`/`$inspect` drops, so neither can anchor a
    // comment. The bound is therefore the last SURVIVING statement's end (not the
    // erased body's last, which may be a trailing dropped `$effect`), defaulting to
    // `content.start` when nothing survives (an import-only script).
    //
    // A comment at or past that bound has no surviving anchor: the oracle
    // re-attaches it into the template (a leading comment of the next emitted node,
    // inside a `$.escape(…)` argument / a component prop object / an `{#if}`
    // condition), a placement this transform can't reproduce — refuse the class.
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
        // A multi-line block comment carries verbatim, but the oracle (esrap)
        // re-indents its interior lines to the emit position, so the two diverge on
        // any interior line whose source indentation differs from the target — refuse
        // until the printer re-indents block-comment interiors to match.
        if comment.multiline {
            return Err(unsupported(Refusal::MultilineBlockComment));
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
/// A module `<script>` is refused before this runs, so the instance script is the
/// only lang-bearing script here. `generics` is refused outright (an open
/// type-parameter *binding*, not annotation erasure), as is any `lang` other than
/// `ts`/`js`/empty.
pub(crate) fn document_ts_flag(root: &Root<'_>, source: &str) -> Result<bool, CompileError> {
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
        Some(RuneInit::StateSnapshot(arg)) => {
            // `const s = $state.snapshot(x)` unwraps to `const s = x`; the binding
            // evaluates through `x` exactly as `x` itself would. A destructured
            // target refuses — the oracle lowers `const {a} = $state.snapshot(x)`
            // into a temp-destructure (`const tmp = x, a = tmp.a`), a shape this
            // transform does not reproduce (a safe over-refusal).
            let name = identifier_binding_name(&declarator.id, source)
                .ok_or_else(|| unsupported(Refusal::DestructuringStateSnapshot))?;
            bindings.insert(
                name,
                Binding {
                    kind: BindingKind::Normal,
                    initial: Initial::Expr(arg),
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
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal:
        // the store rewrite handles them after the loop. The shadow refusal is
        // deferred there too (it needs the full nested-scope set), so pass `None`.
        .allow_store_reads(store_names, None);
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
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal:
        // the store rewrite handles them after the loop. The shadow refusal is
        // deferred there too (it needs the full nested-scope set), so pass `None`.
        .allow_store_reads(store_names, None);
        for expr in guarded {
            walk_expression_guarded(expr, &mut ctx)?;
        }
        return Ok(None);
    }

    let Statement::VariableDeclaration(decl) = stmt else {
        let mut ctx = WalkCtx::new(
            source,
            updated,
            nested_declared,
            derived_names,
            std::rc::Rc::clone(&b.interner),
        )
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal:
        // the store rewrite handles them after the loop. The shadow refusal is
        // deferred there too (it needs the full nested-scope set), so pass `None`.
        .allow_store_reads(store_names, None);
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
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal:
        // the store rewrite handles them after the loop. The shadow refusal is
        // deferred there too (it needs the full nested-scope set), so pass `None`.
        .allow_store_reads(store_names, None);
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
        // Exempt valid `$name` store reads from the guard's `$`-prefixed refusal:
        // the store rewrite handles them after the loop. The shadow refusal is
        // deferred there too (it needs the full nested-scope set), so pass `None`.
        .allow_store_reads(store_names, None);
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
