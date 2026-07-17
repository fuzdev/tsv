//! The sole-blocker refusal census — a diagnostic, collect-don't-bail companion
//! to [`compile`](crate::compile).
//!
//! [`compile`](crate::compile) bails at the **first** unsupported construct, so a
//! corpus run's per-refusal-class counts are first-refusal-only and overstate how
//! much unlocking any one class would yield. [`census`] instead enumerates, per
//! component, **every** refusal class it can independently detect, so a caller can
//! compute — per class — how many files it is the *sole* blocker of (unlocking it
//! yields exactly that many new parity files) versus a *co*-blocker of.
//!
//! # Reuse, never reimplement
//!
//! Every class the census detects is detected by **calling the same guard
//! [`compile`](crate::compile) calls**, never by a copy of its rule:
//!
//! - the standalone analyses are invoked at the natural per-unit granularity (a
//!   guard that bails on the first refusal within a script statement / CSS
//!   selector / template item is invoked once per unit, so the union across units
//!   is collected without threading a sink through the recursive walk), and
//! - [`css_scope::analyze_style`](crate::css_scope::analyze_style) is the one
//!   guard parameterized to collect (its four early-returns push-and-continue when
//!   a sink is present) — its gated path is byte-identical when the sink is absent.
//!
//! The census never touches [`compile`](crate::compile)'s path: it is a separate
//! pass over a freshly-parsed component.
//!
//! # Scope (what is detected vs. disclaimed)
//!
//! Detected independently: the compile-options guards; the structural top-level
//! guards (`<script context="module">`, `<svelte:options>`); the document
//! `lang="ts"` gate; TypeScript erasure's refuse-don't-erase set and the
//! no-`lang="ts"` gate; CSS selector/rule refusals; the instance-script rune
//! rewrites, guards, exports, and invalid imports; the `needs_context`
//! member/call classification; the snippet hoist analysis; the comment
//! carry-through classes [`collect_script_comments`](crate::script_rewrite::collect_script_comments)
//! owns; and the template `TemplateNode` refusals (non-head `<svelte:*>` special
//! elements, `{@debug}`, declaration tags) via the shared fragment seam.
//! `{@render}` is **supported** (a handled arm, not a refusal), so it is
//! deliberately not flagged.
//!
//! **Disclaimed** — detected only as a real first-refusal by the caller composing
//! [`compile`](crate::compile), never independently by the census (so a file whose
//! first-refusal is one of these may hide an *undetected* co-blocker, which is why
//! the caller must surface the exposure count):
//!
//! - the static-evaluator / overlay family (`static evaluation/fold not portable`,
//!   `{@html}` static value, the dynamic-component overlay branch, and the
//!   comment-in-erased-type-region template half) — replicating the evaluator's
//!   overlay push/pop sequence risks a false fold verdict, hence a false sole
//!   blocker (the over-promise direction);
//! - the CSS **matching** refusals — `CssSelectorNoMatch`,
//!   `CssDynamicAttributeMatch`, the element-side `CssCaseInsensitiveNonAscii`, and a
//!   snippet-crossing `CssCombinatorSelector` — which need the upfront element census
//!   plus the selector match (`match_scope`) the census does not run (it only walks
//!   `analyze_style`'s parse-time sink, so it surfaces the *selector-shape* CSS
//!   refusals but not the *matching* ones);
//! - the emitter refusals that read live per-emission state (the block-scope
//!   overlays / the per-`{#each}` name counters / `animate_host_span`): the
//!   styled-component attribute refusals, the `bind:`/event/value attribute refusals,
//!   the block-placement refusals (`{@const}` placement, nested `{#each}`,
//!   generated-name collisions, transition/animate conflicts, snippet/head hoist
//!   order), and the component invocation refusals; and
//! - the pipeline-inline comment-family refusals gated on `has_comments` **and** a
//!   fragment predicate (comments alongside a block / component / `$derived` / a
//!   multi-declarator / hoisted imports / `$$slots`, and comments inside a
//!   rewritten rune region).
//!
//! [`census_detected_buckets`] is the single source of truth for which
//! [`bucket_key`](crate::Refusal::bucket_key)s the census attempts; the caller
//! uses it to size the exposure line.

use std::borrow::Cow;
use std::collections::HashSet;

use tsv_svelte::ast::internal::{Fragment, FragmentNode, Root, SpecialElementKind};
use tsv_ts::ast::internal::Statement;

use crate::analyze::{Bindings, NameSet};
use crate::attr_refs::each_child_fragment;
use crate::build::Builder;
use crate::css_scope::analyze_style;
use crate::fragment::fragment_node_kind;
use crate::needs_context::analyze_component;
use crate::script_rewrite::{
    analyze_script, collect_script_comments, document_ts_flag, plain_identifier_name,
    refuse_runes_invalid_import, refuse_template_typescript, rewrite_script_statement,
};
use crate::snippet::analyze_snippets;
use crate::{CompileError, CompileOptions, Generate, Refusal, erase};

/// Enumerate every refusal class the census can independently detect for
/// `source`, deduplicated by [`bucket_key`](Refusal::bucket_key).
///
/// A real parse error is the one early exit (there is nothing to census); every
/// other outcome returns the detected set. See the module documentation for the
/// detected-vs-disclaimed split and the reuse contract.
///
/// **Not panic-guarded.** This runs the reused guards directly, with no
/// `catch_unwind` — it relies on those guards being panic-free (they are on the
/// whole corpus). A per-file panic is caught only by the corpus harness's own
/// `--profile corpus` unwind boundary, not here; a non-harness caller gets no
/// catch, so a panic propagates.
///
/// # Errors
///
/// [`CompileError::Parse`] when `source` is not a parseable Svelte component.
pub fn census(source: &str, options: &CompileOptions) -> Result<Vec<Refusal>, CompileError> {
    let mut found: Vec<Refusal> = Vec::new();

    // Compile-options guards — [`compile`](crate::compile)'s first two checks.
    if options.generate == Generate::Client {
        found.push(Refusal::ClientGeneration);
    }
    if options.dev {
        found.push(Refusal::DevMode);
    }

    let arena = bumpalo::Bump::new();
    let root = tsv_svelte::parse(source, &arena)?;
    collect(&root, source, &arena, &mut found);

    Ok(dedup_by_bucket(found))
}

/// The set of [`bucket_key`](Refusal::bucket_key)s the census attempts to detect
/// independently — the single source of truth for the exposure line.
///
/// Built by projecting representative variants through
/// [`bucket_key`](Refusal::bucket_key) rather than by hard-coding strings, so a
/// reworded bucket key can never drift this list out of sync. A caller computes
/// the **exposure** (files whose first-refusal hides a possibly-undetected
/// co-blocker) as the candidates whose first-refusal bucket key is **not** in this
/// set.
#[must_use]
pub fn census_detected_buckets() -> Vec<Cow<'static, str>> {
    // Representative instances — parameters collapse in the bucket key, so any
    // placeholder value is fine.
    let s = String::new;
    [
        // Options + structural top-level.
        Refusal::ClientGeneration,
        Refusal::DevMode,
        Refusal::ModuleScript,
        Refusal::SvelteOptions,
        // Document TypeScript gate.
        Refusal::LangInstanceScript { lang: s() },
        Refusal::GenericsAttribute,
        // TypeScript erasure: refuse-don't-erase + the no-`lang="ts"` gate.
        Refusal::TypeScriptWithoutLangTs,
        Refusal::TsEnum,
        Refusal::TsNamespaceWithValue,
        Refusal::TsDottedNamespace,
        Refusal::TsParameterProperty,
        Refusal::Decorator,
        Refusal::TsAccessorField,
        Refusal::TsAbstractProperty,
        Refusal::TsOverloadSignature,
        Refusal::TsIndexSignature,
        Refusal::TsImportEquals,
        Refusal::TsExportAssignment,
        Refusal::TsNamespaceExport,
        // CSS scoping.
        Refusal::CssAtRule,
        Refusal::CssNestedRule,
        Refusal::CssEmptyRule,
        Refusal::CssCombinatorSelector,
        Refusal::CssUnsupportedSelector,
        // Its selector-side (non-ASCII name/value) is an `analyze_style` sink
        // refusal; the element-side (non-ASCII attr/element name/value) is an
        // emission-time refusal, disclaimed like `CssDynamicAttributeMatch`.
        Refusal::CssCaseInsensitiveNonAscii,
        // Instance-script rune rewrites / guards / scaffold.
        Refusal::InstanceScriptExport,
        Refusal::LegacyReactiveStatement,
        Refusal::SvelteInternalImport,
        Refusal::RunesInvalidImport { name: s() },
        Refusal::Rune { name: s() },
        Refusal::DollarPrefixedIdentifier { name: s() },
        Refusal::DerivedBindingRead { name: s() },
        Refusal::TopLevelAwait,
        Refusal::DestructuringState,
        Refusal::DestructuringDerived,
        Refusal::DestructuringDerivedBy,
        Refusal::PropsBindingPattern,
        Refusal::BindingPatternShape { kind: "" },
        Refusal::CommentsWithArglessState,
        Refusal::CommentsWithRestProps,
        Refusal::CommentsWithNonDestructuredProps,
        // needs_context member/call classification.
        Refusal::MemberCallAmbiguousRoot { name: s() },
        Refusal::MemberCallEscapedRoot,
        // Snippet hoist analysis.
        Refusal::DuplicateSnippetName { name: s() },
        Refusal::SnippetHoistAmbiguous { name: s() },
        // Comment carry-through (the classes collect_script_comments owns).
        Refusal::TemplateComments,
        Refusal::CommentAfterLastStatement,
        Refusal::LeadingCommentGluedToScript,
        Refusal::FormatIgnoreComment,
        Refusal::CommentsWithTemplateBeforeScript,
        // Template nodes the shared fragment seam detects (the `other =>` refusal
        // arm). Each kind is its own bucket key, so all detected kinds are listed.
        Refusal::TemplateNode {
            kind: "special element",
        },
        Refusal::TemplateNode {
            kind: "{@debug} tag",
        },
        Refusal::TemplateNode {
            kind: "declaration tag",
        },
    ]
    .iter()
    .map(Refusal::bucket_key)
    .collect()
}

/// Run every collector over the parsed component, pushing each detected refusal.
///
/// Ordering mirrors [`compile_server`](crate::transform_server) closely enough to
/// reuse its guards on the same inputs — but every step catches its `Err` and
/// keeps going, so one file yields its whole detectable blocker set.
fn collect<'arena>(
    root: &Root<'arena>,
    source: &str,
    arena: &'arena bumpalo::Bump,
    found: &mut Vec<Refusal>,
) {
    // Structural top-level guards. These are field-presence facts, not rules with
    // hidden state — reproduced directly (a shared extraction would only wrap a
    // `.is_some()`), matching `compile_server`'s first two bails.
    if root.module.is_some() {
        found.push(Refusal::ModuleScript);
    }
    if root.options.is_some() {
        found.push(Refusal::SvelteOptions);
    }

    // Document `lang="ts"` gate (reused verbatim). On refusal, treat the document
    // as non-TS for the rest of the pass (conservative).
    let ts_document = match document_ts_flag(root, source) {
        Ok(flag) => flag,
        Err(err) => {
            push_unsupported(found, err);
            false
        }
    };

    // TypeScript erasure. Per-statement so every refuse-don't-erase site is
    // collected (a whole-body run bails on the first), and so the best-effort
    // erased body skips only the offending statement. `erase_statements` is
    // reused unmodified — the census never rebuilds its output.
    //
    // Best-effort: an erase-FAILED statement is dropped from the body fed to the
    // downstream analyses (`analyze_script`/`analyze_component`/the script loop).
    // So on a file ALREADY carrying an erase refusal, a co-blocker those analyses
    // would have found in the dropped statement can shift — diagnostic-acceptable,
    // and such a file is already exposed by its erase refusal.
    let mut erased = bumpalo::collections::Vec::new_in(arena);
    let mut any_typescript = false;
    if let Some(script) = root.instance {
        for stmt in script.content.body {
            match erase::erase_statements(arena, source, std::slice::from_ref(stmt)) {
                Ok(er) => {
                    any_typescript |= er.typescript;
                    erased.extend_from_slice(er.body);
                }
                Err(err) => push_unsupported(found, err),
            }
        }
    }
    let erased_body: &'arena [Statement<'arena>] = erased.into_bump_slice();
    // The document-wide no-`lang="ts"` gate: the script half (any erased TS syntax
    // in a non-TS document) and the template half (reused verbatim, bails on the
    // first — one `TypeScriptWithoutLangTs` bucket per file regardless).
    if !ts_document {
        if any_typescript {
            found.push(Refusal::TypeScriptWithoutLangTs);
        }
        if let Err(err) = refuse_template_typescript(root, source, arena) {
            push_unsupported(found, err);
        }
    }

    // CSS scoping — the one parameterized guard: the sink collects all four
    // selector/rule refusals instead of bailing on the first.
    if let Some(style) = root.css {
        let mut css = Vec::new();
        let _ = analyze_style(style, source, Some(&mut css));
        found.append(&mut css);
    }

    // Comment carry-through (reused verbatim). `has_comments` feeds the script
    // rewrite exactly as `compile_server` computes it.
    let has_comments = match collect_script_comments(root, source, erased_body) {
        Ok(comments) => !comments.is_empty(),
        Err(err) => {
            push_unsupported(found, err);
            false
        }
    };

    // Binding table + derived names (reused verbatim; best-effort on refusal).
    let mut bindings = Bindings::empty();
    let mut derived_names = NameSet::default();
    if let Err(err) = analyze_script(erased_body, source, &mut bindings, &mut derived_names) {
        push_unsupported(found, err);
    }

    // `needs_context` member/call classification (reused verbatim). It walks the
    // raw fragment, exactly as `compile_server` does. Only `uses_slots` is read
    // out here (for the rewrite below); the MemberCall refusal is captured on Err.
    let uses_slots = match analyze_component(root, source, erased_body) {
        Ok(ctx) => ctx.uses_slots,
        Err(err) => {
            push_unsupported(found, err);
            false
        }
    };

    // Snippet hoist analysis (reused verbatim). Its two inputs mirror
    // `compile_server`: the instance binding names and the subset that are imports.
    let import_names = import_local_names(erased_body, source);
    let instance_binding_names: NameSet = bindings.names().map(str::to_string).collect();
    if let Err(err) = analyze_snippets(root, source, &instance_binding_names, &import_names) {
        push_unsupported(found, err);
    }

    // Instance-script phase: exports, invalid imports, and the per-statement rune
    // rewrite/guard (reused verbatim), collected across statements. A scratch
    // builder absorbs the rewrite's appendix minting; its output is discarded.
    let mut b = Builder::new(arena, source, std::rc::Rc::clone(&root.interner));
    let mut updated = NameSet::default();
    let mut nested_declared = NameSet::default();
    let mut uses_props = false;
    let mut has_effects = false;
    let mut dropped_regions = Vec::new();
    for stmt in erased_body {
        if matches!(
            stmt,
            Statement::ExportNamedDeclaration(_)
                | Statement::ExportDefaultDeclaration(_)
                | Statement::ExportAllDeclaration(_)
                | Statement::TSNamespaceExportDeclaration(_)
                | Statement::TSExportAssignment(_)
        ) {
            found.push(Refusal::InstanceScriptExport);
            continue;
        }
        if let Statement::ImportDeclaration(import) = stmt {
            if let Err(err) = refuse_runes_invalid_import(import, source) {
                push_unsupported(found, err);
            }
            continue;
        }
        if let Err(err) = rewrite_script_statement(
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
        ) {
            push_unsupported(found, err);
        }
    }

    // Template special elements (`{@render}`/`{@debug}`/`<svelte:*>` in an
    // emitted position). Reuses the shared fragment recursion seam
    // (`each_child_fragment`) and the emitter's own kind labeller
    // (`fragment_node_kind`), so the `TemplateNode` bucket matches
    // `fragment::clean_and_split`'s `other =>` arm exactly. A supported bare
    // `<svelte:head>` is excluded, as it is there.
    collect_template_nodes(&root.fragment, found);
}

/// The import local names of an erased instance body — the subset of instance
/// bindings that do not disqualify snippet hoisting. Mirrors `compile_server`.
fn import_local_names(body: &[Statement<'_>], source: &str) -> NameSet {
    use tsv_ts::ast::internal::ImportSpecifier;
    body.iter()
        .filter_map(|stmt| match stmt {
            Statement::ImportDeclaration(import) => Some(import),
            _ => None,
        })
        .flat_map(|import| import.specifiers)
        .filter_map(|spec| {
            let local = match spec {
                ImportSpecifier::Default(s) => &s.local,
                ImportSpecifier::Named(s) => &s.local,
                ImportSpecifier::Namespace(s) => &s.local,
            };
            plain_identifier_name(local, source)
        })
        .collect()
}

/// Push the `TemplateNode` refusals a fragment holds, recursing every child
/// fragment via the shared seam.
///
/// Mirrors [`fragment::emit_fragment`](crate::fragment)'s special-element handling:
/// a special element refuses as `template node special element` only when it is
/// **neither** `<svelte:head>` **nor** one of the SSR-inert kinds
/// (`<svelte:window>`/`<svelte:body>`/`<svelte:document>`) — so
/// `<svelte:element>`/`<svelte:component>`/`<svelte:self>`/`<slot>`/… still refuse,
/// but a valid top-level window/body/document (which now compiles to nothing) does
/// not. A `{@debug}` or declaration tag refuses; a bare `<svelte:head>`, a
/// `{@render}` tag, and every other node are SUPPORTED (their own handled arms), so
/// they are not refusals — treating `{@render}` as one would fabricate a co-blocker
/// on every component that renders a snippet.
///
/// (A node inside a dropped `{:catch}` branch is not emitted, so the emitter
/// never refuses it there; walking every child fragment can therefore
/// over-detect in that doubly-rare position — a special element AND only in a
/// `{:catch}` — accepted as a diagnostic imprecision. Likewise a window/body/
/// document at an INVALID position — nested, or a duplicate — compiles-refuses
/// (`SpecialElementInvalidPlacement`/`DuplicateSpecialElement`) but is not
/// re-detected here; such input is oracle-rejected, so it is never a parity
/// candidate.)
fn collect_template_nodes(fragment: &Fragment<'_>, found: &mut Vec<Refusal>) {
    for node in fragment.nodes {
        let refused = match node {
            FragmentNode::SpecialElement(se) => !matches!(
                se.kind,
                SpecialElementKind::SvelteHead
                    | SpecialElementKind::SvelteWindow
                    | SpecialElementKind::SvelteBody
                    | SpecialElementKind::SvelteDocument
            ),
            FragmentNode::DebugTag(_) | FragmentNode::DeclarationTag(_) => true,
            _ => false,
        };
        if refused {
            found.push(Refusal::TemplateNode {
                kind: fragment_node_kind(node),
            });
        }
        each_child_fragment(node, &mut |child| collect_template_nodes(child, found));
    }
}

/// Push a [`CompileError::Unsupported`]'s [`Refusal`]; any other error variant is
/// impossible from the guards the census calls (they only ever refuse), so it is
/// silently ignored rather than surfaced.
fn push_unsupported(found: &mut Vec<Refusal>, err: CompileError) {
    if let CompileError::Unsupported(reason) = err {
        found.push(reason);
    }
}

/// Deduplicate by [`bucket_key`](Refusal::bucket_key), preserving first-seen
/// order — the census reports a *set* of blocker classes, so a file with three
/// unsupported selectors contributes one `CssUnsupportedSelector`.
fn dedup_by_bucket(refusals: Vec<Refusal>) -> Vec<Refusal> {
    let mut seen: HashSet<String> = HashSet::new();
    refusals
        .into_iter()
        .filter(|r| seen.insert(r.bucket_key().into_owned()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;

    fn bucket_set(source: &str) -> Vec<String> {
        let mut keys: Vec<String> = census(source, &CompileOptions::default())
            .expect("census parses")
            .iter()
            .map(|r| r.bucket_key().into_owned())
            .collect();
        keys.sort();
        keys
    }

    #[test]
    fn two_independent_blockers_are_both_detected() {
        // An unsupported CSS selector (CSS analysis) AND a `<script context="module">`
        // (structural top-level) — two independent dimensions, so the census must
        // return BOTH, where `compile()` would bail on only the first.
        let source = "<script context=\"module\">let x = 1;</script>\n\
                      <style>:has(.x) { color: red; }</style>\n";
        let keys = bucket_set(source);
        assert!(
            keys.iter().any(|k| k.contains("module <script")),
            "module script not detected: {keys:?}"
        );
        assert!(
            keys.iter().any(|k| k.contains("unsupported css selector")),
            "unsupported css selector not detected: {keys:?}"
        );
        // And `compile()` bails on exactly one of them — the census is strictly
        // more informative.
        assert!(matches!(
            compile(source, &CompileOptions::default()),
            Err(CompileError::Unsupported(_))
        ));
    }

    #[test]
    fn multiple_css_refusals_collect_not_bail() {
        // Two distinct CSS refusals in one stylesheet: a `||` column combinator
        // (unsupported) and an unsupported pseudo compound (`:has(.x)`). The
        // parameterized `analyze_style` sink must surface both, not just the first.
        let source = "<style>a || b { color: red; }\n:has(.x) { color: blue; }</style>\n";
        let keys = bucket_set(source);
        assert!(
            keys.iter().any(|k| k.contains("combinator")),
            "combinator selector not detected: {keys:?}"
        );
        assert!(
            keys.iter().any(|k| k.contains("unsupported css selector")),
            "unsupported selector not detected: {keys:?}"
        );
    }

    #[test]
    fn single_blocker_is_the_only_class() {
        // A lone unsupported CSS selector, nothing else unsupported — the census
        // returns exactly that one class (the SOLE-blocker shape).
        let source = "<style>:has(.x) { color: red; }</style>\n";
        let keys = bucket_set(source);
        assert_eq!(
            keys,
            vec![
                "unsupported css selector in <style> (:global/:is/:where/:has/:not/:root/nesting)"
                    .to_string()
            ],
            "expected exactly one blocker class: {keys:?}"
        );
    }

    #[test]
    fn special_element_is_detected() {
        // A `<svelte:element>` — the parity-menu special-element class, via the
        // shared fragment seam.
        let source = "<svelte:element this={tag}>hi</svelte:element>\n";
        let keys = bucket_set(source);
        assert!(
            keys.iter()
                .any(|k| k.contains("template node special element")),
            "special element not detected: {keys:?}"
        );
    }

    #[test]
    fn ssr_inert_special_element_is_not_detected() {
        // A top-level `<svelte:window>` compiles (emits nothing), so it must NOT
        // census as `template node special element` — unlike `<svelte:element>`,
        // which still refuses. A `<svelte:body>` beside it must not appear either.
        let source = "<svelte:window onkeydown={h} /><svelte:body use:act />\n";
        assert!(
            compile(source, &CompileOptions::default()).is_ok(),
            "sanity: SSR-inert special elements must compile"
        );
        let keys = bucket_set(source);
        assert!(
            !keys.iter().any(|k| k.contains("special element")),
            "SSR-inert special element wrongly censused: {keys:?}"
        );
    }

    #[test]
    fn supported_component_has_no_blockers() {
        // A component the compiler fully supports must census clean (empty set) —
        // the census never invents a refusal for a compilable shape.
        let source = "<h1>hello</h1>\n";
        assert!(
            compile(source, &CompileOptions::default()).is_ok(),
            "sanity: this must compile"
        );
        assert!(
            bucket_set(source).is_empty(),
            "clean component censused blockers: {:?}",
            bucket_set(source)
        );
    }

    #[test]
    fn gated_path_still_bails_on_first_css_refusal() {
        // The census parameterized `analyze_style` with a collect sink. With the
        // sink ABSENT (the `compile()` path) the four checks must stay
        // bail-on-first, byte-identical to before — so `compile()` on a stylesheet
        // with two CSS refusals surfaces exactly the FIRST (the `||` combinator),
        // never the collected pair the census would return.
        let source = "<style>a || b { color: red; }\n:has(.x) { color: blue; }</style>\n";
        match compile(source, &CompileOptions::default()) {
            Err(CompileError::Unsupported(Refusal::CssCombinatorSelector)) => {}
            other => panic!("gated path must bail on the first CSS refusal, got {other:?}"),
        }
        // And a supported single-class-selector style still compiles unchanged.
        let ok = "<div class=\"foo\">x</div>\n<style>.foo { color: red; }</style>\n";
        let out = compile(ok, &CompileOptions::default()).expect("supported style compiles");
        assert!(out.css.is_some(), "scoped CSS must be produced");
    }

    #[test]
    fn detected_buckets_are_unique() {
        // The exposure-line source of truth must have no duplicate keys.
        let buckets = census_detected_buckets();
        let mut seen = HashSet::new();
        for b in &buckets {
            assert!(seen.insert(b.clone()), "duplicate detected bucket: {b}");
        }
    }
}
