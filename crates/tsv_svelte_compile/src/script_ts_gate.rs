//! The document-wide TypeScript flag, the template half of its gate, and the
//! erase self-check that closes the loop on the finished program.
//!
//! Oracle phase 1: Svelte decides TypeScript at *parse* time, for the whole
//! document at once, before it chooses what to emit — so this is
//! target-independent and applies equally to a server and a client transform.
//! See [`crate::transform_server`] for the orchestration that calls these in
//! sequence, and [`crate::erase`] for the erasure these gate.

use tsv_lang::Span;
use tsv_svelte::ast::internal::{AttributeNode, AttributeValue, Root};
use tsv_ts::ast::internal::Statement;

use crate::attr_refs::{TemplateItem, each_template_item};
use crate::transform_server::unsupported;
use crate::{CompileError, Refusal, erase};

/// Assert no TypeScript-only node survived into the emitted program.
///
/// Both halves of the erasure — the instance script's `Program` and each
/// template expression at its borrow point — run before this, so **any**
/// survivor is a compiler bug: an erase case missed, or a borrow point that
/// never called [`EmitEnv::erase`](crate::transform_server::EmitEnv::erase). It is
/// surfaced loudly as
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
            let name = attr.name(source).to_string();
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

/// The oracle's parse-time `<script>` attribute rules, all raised in one
/// source-order loop (`1-parse/read/script.js:48-79`, first error wins):
///
/// - **`script_reserved_attribute`** — the FIRST check: a `<script>` attribute
///   named `server`, `client`, `worker`, `test`, or `default` (the oracle's
///   `RESERVED_ATTRIBUTES`) is rejected regardless of its value (`<script server>`
///   and `<script server="x">` both fail).
/// - **`script_invalid_context`** — a `context` attribute is valid ONLY as a single
///   Text value `"module"` (the legacy spelling of `<script module>`). A boolean
///   `context`, an expression `context={x}`, a multi-chunk value, or any other text
///   (`context="default"`, …) is rejected.
/// - **`script_invalid_attribute_value`** — a `module` attribute must be a plain
///   BOOLEAN (`<script module>`); the oracle rejects `attribute.value !== true`, so
///   `module="foo"`, `module="module"`, `module=""`, and `module={x}` all fail.
///
/// ⚠️ The oracle's `script_unknown_attribute` (any name outside the reserved five
/// and the allowed `context`/`generics`/`lang`/`module`) is only a WARNING, so an
/// unknown attribute (`<script foo>`) still COMPILES — this pass refuses only the
/// closed reserved set, never an unknown name.
///
/// tsv's parser (`detect_script_context` in `tsv_svelte`) routes to the module slot
/// only for `context="module"` or a value-less `module`, treating every other form
/// as an ordinary instance script — so all three invalid shapes reach here as an
/// accepted component the oracle rejects, and each is refused. Checked on **both**
/// scripts: `<script module context="foo">` is a module script to tsv, yet the
/// oracle still rejects its `context="foo"`, and a reserved attribute on either
/// script rejects.
///
/// The rules share ONE per-attribute pass — not three — so the refusal REASON
/// matches the oracle's first-error-wins order within a script: `<script server
/// module="x">` reports the reserved `server` (source-first), `<script module="x"
/// context="y">` the `module` value, `<script context="y" module="x">` the
/// `context`. Reserved names are disjoint from `module`/`context`, so at most one
/// rule fires per attribute.
pub(crate) fn refuse_invalid_script_attributes(
    root: &Root<'_>,
    source: &str,
) -> Result<(), CompileError> {
    for script in [root.module, root.instance].into_iter().flatten() {
        for attr_node in script.attributes {
            let AttributeNode::Attribute(attr) = attr_node else {
                continue;
            };
            let name = attr.name(source).to_string();
            match name.as_str() {
                // The oracle's FIRST check, fired before module/context and
                // regardless of the attribute's value.
                "server" | "client" | "worker" | "test" | "default" => {
                    return Err(unsupported(Refusal::ScriptReservedAttribute { name }));
                }
                "context" => {
                    // The oracle's `is_text_attribute(attr) && attr.value[0].data ===
                    // 'module'` — a single Text node equal to "module". A boolean
                    // (`value` is `None`), an expression, a multi-chunk value, or any
                    // other text fails.
                    let valid = matches!(
                        attr.value,
                        Some([AttributeValue::Text(text)]) if text.data(source) == "module"
                    );
                    if !valid {
                        return Err(unsupported(Refusal::ScriptInvalidContext));
                    }
                }
                "module" => {
                    // The oracle's `attribute.value !== true` — a boolean carries no
                    // value (`None`); any value (`Some(_)`, incl. an empty string)
                    // refuses.
                    if attr.value.is_some() {
                        return Err(unsupported(Refusal::ScriptInvalidAttributeValue));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(())
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
/// The borrow points ([`EmitEnv::erase`](crate::transform_server::EmitEnv::erase))
/// already erase every template expression
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
