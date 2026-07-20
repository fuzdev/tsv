//! Emission-independent validation: duplicate attributes, node placement, names.

use super::support::*;
use crate::*;

/// A component import, so a `<N>` below is a Component rather than an unknown tag.
const SLOT_IMPORT: &str = "<script>import N from './N.svelte';</script>";

#[test]
fn compile_duplicate_attribute_refuses() {
    // The oracle's parse-time `attribute_duplicate` (`1-parse/state/element.js:250`)
    // — a rule tsv did not enforce, so these all compiled. Each of the three
    // participating attribute kinds keys separately.
    assert_unsupported("<div id=\"a\" id=\"b\"></div>", "duplicate");
    assert_unsupported("<div class:a class:a></div>", "duplicate");
    assert_unsupported(
        "<div style:color=\"a\" style:color=\"b\"></div>",
        "duplicate",
    );
    // A `bind:` normalizes onto `Attribute`, so it collides with the plain name.
    assert_unsupported("<input value=\"1\" bind:value={v} />", "duplicate");
    // It fires in an SSR-DROPPED region too: the oracle raises it at PARSE, long
    // before it decides what to emit.
    assert_unsupported(
        "{#await p}<i>x</i>{:catch e}<div id=\"a\" id=\"b\"></div>{/await}",
        "duplicate",
    );
}

#[test]
fn compile_duplicate_attribute_accepts_the_oracle_s_exemptions() {
    // ⚠️ The must-NOT-over-refuse half. Each of these is legal to the oracle, and
    // an over-eager port of the rule breaks one of them.
    // Different KINDS of the same name are different keys.
    let _ = compile_js("<div class:a style:a=\"1\" a=\"2\"></div>");
    // Only Attribute/Bind/Style/Class participate — `use:` may legally repeat.
    let _ = compile_js("<script>let x, y;</script><div use:x use:y></div>");
    // `this` is never recorded, which is what makes this shape legal.
    let _ = compile_js(
        "<script>let el;</script><svelte:element this=\"div\" bind:this={el}></svelte:element>",
    );
    // The rule is per-element, not per-document.
    let _ = compile_js("<div class:a></div><div class:a></div>");
}

#[test]
fn compile_root_only_meta_tag_placement_refuses() {
    // `svelte_meta_invalid_placement` / `svelte_meta_duplicate` for `<svelte:head>`
    // — tsv already enforced both for the SSR-inert tags, but not for head.
    assert_unsupported(
        "<div><svelte:head><title>x</title></svelte:head></div>",
        "top-level",
    );
    // Any container counts, including a `<svelte:boundary>` — the placement test is
    // "direct child of Root", so a boundary between makes it invalid.
    assert_unsupported(
        "<svelte:boundary><svelte:head><title>x</title></svelte:head></svelte:boundary>",
        "top-level",
    );
    assert_unsupported(
        "<svelte:head><title>a</title></svelte:head><svelte:head><title>b</title></svelte:head>",
        "duplicate",
    );
    // One at the root is of course still fine.
    let _ = compile_js("<svelte:head><title>a</title></svelte:head>");
}

#[test]
fn compile_unknown_svelte_meta_tag_refuses() {
    // The oracle's parse-time `svelte_meta_invalid_tag` (`element.js:142`): a
    // `svelte:`-prefixed element whose name is not a known meta tag. tsv's parser
    // routes every known `svelte:` name to a `SpecialElementKind` (and
    // `svelte:options` to `Root.options`), so an unknown one reaches a regular
    // element and the compiler refuses it.
    assert_unsupported("<svelte:selfdestructive x=\"a\" />", "meta tag");
    assert_unsupported("<svelte:nope />", "meta tag");
    // Fires wherever the tag sits — inside a block, and inside a region SSR DROPS
    // (a `{:catch}` branch), the whole reason this rule lives in `validate.rs`
    // rather than at an emitter.
    assert_unsupported("{#if x}<svelte:selfdestructive />{/if}", "meta tag");
    assert_unsupported(
        "{#await p}a{:then v}b{:catch e}<svelte:nope />{/await}",
        "meta tag",
    );

    // Controls — the rule is `svelte:`-specific and never touches a KNOWN meta tag.
    // `<svelte:head>`/`<svelte:window>`/`<svelte:element>`/`<svelte:boundary>` all
    // parse to special kinds, so they compile (or refuse for their OWN reasons, not
    // this one); and a non-`svelte:` namespaced tag is an ordinary regular element.
    let _ = compile_js("<svelte:window />");
    let _ = compile_js("<svelte:element this=\"div\" />");
    let _ = compile_js("<svelte:boundary>x</svelte:boundary>");
    let _ = compile_js("<foo:bar />");
}

#[test]
fn compile_invalid_node_placement_refuses() {
    // The oracle's `node_invalid_placement` — markup a browser would REPAIR, which
    // breaks Svelte's assumptions about component structure. Every case below was
    // confirmed oracle-REJECTED with `canonical_compile`.
    // ⚠️ The DIRECT `<p><div>` case is unreachable: both parsers apply HTML
    // tag-omission and auto-close the `<p>`, so no parent/child relation forms
    // (tsv then parse-errors on the stray `</p>`). That is why every oracle sample
    // puts an element in between and trips the ANCESTOR rule instead.
    assert_unsupported("<div><p><span><div>x</div></span></p></div>", "descendant");
    assert_unsupported(
        "<a href=\"/x\"><div><a href=\"/y\">z</a></div></a>",
        "descendant",
    );
    assert_unsupported("<div><tbody></tbody></div>", "must be the child of a");
    // The fallback switch: a tag legal only under a special-parsing parent.
    assert_unsupported("<div><caption>x</caption></div>", "cannot be a child of");
    // `#text` and `{expression}` reach the parent test through their own visitors.
    assert_unsupported(
        "<table><tbody>text</tbody></table>",
        "only allows these children",
    );
    assert_unsupported(
        "<script>let v = 1;</script><table><tbody>{v}</tbody></table>",
        "only allows these children",
    );
    // It fires in an SSR-DROPPED region too — the oracle validates before it
    // decides what to emit.
    assert_unsupported(
        "{#await p}<i>x</i>{:catch e}<div><p><span><div>y</div></span></p></div>{/await}",
        "descendant",
    );
}

#[test]
fn compile_invalid_node_placement_respects_the_oracle_s_escape_hatches() {
    // ⚠️ The must-NOT-over-refuse half. Each case is oracle-ACCEPTED (verified with
    // `canonical_compile`), and a plausible over-eager port breaks one of them.

    // A block downgrades the violation to a WARNING (`node_invalid_placement_ssr`):
    // Svelte compiles each block into its own template string, so it works
    // client-side. A refusal here would reject ordinary real components. The
    // control is the identical shape MINUS the block, asserted refused above —
    // so this pair isolates the downgrade rather than merely finding a legal
    // document.
    let _ = compile_js("<div><p><span>{#if true}<div>y</div>{/if}</span></p></div>");

    // A custom element may contain anything and go anywhere.
    let _ = compile_js("<foo-bar><div>x</div></foo-bar>");
    let _ = compile_js("<p><my-thing>x</my-thing></p>");

    // `<template>`'s immediate children are exempt outright.
    let _ = compile_js("<template><tbody></tbody></template>");

    // `reset_by`: a `<dl>` (or a custom element) re-opens the `dt`/`dd` descendants.
    let _ = compile_js("<dl><dd><dl><dt>x</dt></dl></dd></dl>");
    let _ = compile_js("<dl><dd><foo-bar><dt>x</dt></foo-bar></dd></dl>");

    // Whitespace-only text is not checked (`regex_not_whitespace` is the narrow
    // `[^ \t\r\n]`, so only these four characters count as whitespace).
    let _ = compile_js("<table><tbody> </tbody></table>");

    // A valid table nest must survive the `only` allow-lists intact.
    let _ = compile_js("<table><tbody><tr><td>x</td></tr></tbody></table>");

    // A component BREAKS the ancestor walk and resets `parent_element`, so its
    // children are validated against the component, not the outer `<p>`.
    let _ = compile_js(
        "<script>import Foo from './Foo.svelte';</script><p><Foo><div>x</div></Foo></p>",
    );
}

#[test]
fn compile_invalid_node_placement_gates_the_custom_element_reset_on_reset_by() {
    // ⭐ The one transcription trap. The oracle gates its whole reset scan — and the
    // custom-element short-circuit INSIDE it — on `reset_by` being PRESENT. Only
    // `dt`/`dd` carry one, so a custom element rescues a `dt` chain (asserted above)
    // but does NOT rescue a `<p>` descendant, whose entry has no `reset_by`.
    // Hoisting that short-circuit out of the guard silently under-refuses this.
    // Confirmed oracle-REJECTED.
    assert_unsupported("<p><foo-bar><div>x</div></foo-bar></p>", "descendant");
}

#[test]
fn compile_refuses_an_invalid_attribute_name() {
    // The oracle's `attribute_invalid_name`
    // (`2-analyze/visitors/shared/element.js:59`), whose regex has TWO alternatives:
    // an illegal LEADING character, and an illegal character anywhere.
    // Each shape below is live-verified oracle-REJECTED.

    // Leading alternative — `^[0-9-.]`.
    assert_unsupported("<p 3aa=\"abc\">x</p>", "attribute name");
    assert_unsupported("<p -a>x</p>", "attribute name");
    assert_unsupported("<p .a>x</p>", "attribute name");

    // Anywhere alternative — the punctuation class.
    assert_unsupported("<p a*a>x</p>", "attribute name");
    assert_unsupported("<p a;=\"abc\">x</p>", "attribute name");
    assert_unsupported("<p }>x</p>", "attribute name");

    // `<svelte:element>` shares `validate_element`, so it is in scope too.
    assert_unsupported(
        "<svelte:element this=\"p\" 3aa=\"x\">y</svelte:element>",
        "attribute name",
    );
}

#[test]
fn compile_accepts_attribute_names_the_oracle_allows() {
    // ⚠️ The negatives are the load-bearing half — an over-REFUSAL here would turn
    // ordinary components into refusals, which no ratchet would catch (the
    // validation suites only hold invalid input). All live-verified oracle-ACCEPTED.

    // `.` is illegal only as the LEADING character, never within.
    let _ = compile_js("<p a.b=\"x\">y</p>");
    // A `-` within is the ubiquitous `data-`/`aria-` shape.
    let _ = compile_js("<p data-x=\"1\" aria-label=\"y\">z</p>");
    // A digit within, not leading.
    let _ = compile_js("<p a3=\"x\">y</p>");
}

#[test]
fn compile_does_not_apply_the_attribute_name_rule_to_components() {
    // ⚠️ The oracle calls `validate_element` from `RegularElement.js` and
    // `SvelteElement.js` ONLY — never from the Component visitor — so a component
    // prop may carry a name no element attribute could. Live-verified
    // oracle-ACCEPTED. Widening the rule to components would be an over-refusal
    // that only this test catches.
    let _ = compile_js("<script>import F from './F.svelte';</script><F 3aa=\"abc\" />");
}

#[test]
fn compile_refuses_an_event_handler_without_an_expression_value() {
    // The oracle's `attribute_invalid_event_handler`
    // (`2-analyze/visitors/shared/element.js:64`): an `on…` attribute is legal only
    // with a SINGLE-expression value. Each shape below is live-verified
    // oracle-REJECTED.

    // A text value — the sample the ratchet pinned.
    assert_unsupported("<button onclick=\"foo\">x</button>", "event handler");
    // ⚠️ A BARE handler too (`value === true` fails `is_expression_attribute`), which
    // reads as legal and is not.
    assert_unsupported("<button onclick>x</button>", "event handler");
    // A MULTI-chunk value is not a single expression either.
    assert_unsupported("<button onclick=\"{a}{b}\">x</button>", "event handler");
    // ⚠️ The name test is `length > 2`, so a 3-character name is the boundary.
    assert_unsupported("<button onx>x</button>", "event handler");
    // `<svelte:element>` shares `validate_element`, so it is in scope too.
    assert_unsupported(
        "<svelte:element this=\"button\" onclick=\"foo\" />",
        "event handler",
    );
}

#[test]
fn compile_accepts_event_handlers_the_oracle_allows() {
    // ⚠️ The over-refusal half. All live-verified oracle-ACCEPTED.

    // The ordinary shape.
    let _ = compile_js("<script>let f = () => {};</script><button onclick={f}>x</button>");
    // ⭐ `on` alone is LEGAL — the oracle's `length > 2` guard. Writing the rule as a
    // bare `starts_with(\"on\")` refuses this, and only this test catches it.
    let _ = compile_js("<button on>x</button>");
    // ⚠️ A COMPONENT is exempt: `validate_element` is never reached from the
    // Component visitor, so a prop named `onbar` may carry a text value.
    let _ = compile_js("<script>import F from './F.svelte';</script><F onbar=\"bar\" />");
}

#[test]
fn compile_refuses_an_unquoted_multichunk_attribute_value() {
    // The oracle's `attribute_unquoted_sequence` — the error half of
    // `validate_attribute` (`2-analyze/visitors/shared/attribute.js:41-48`): an
    // UNQUOTED value of two or more chunks must be quoted. Every shape below is
    // live-verified oracle-REJECTED (compile_corpus_compare over a probe dir
    // reported all as `attribute_unquoted_sequence` over-acceptances pre-fix).

    // Text + expression — the common real-world shape (`href=/{path}`).
    assert_unsupported(
        "<script>let path = $state('a');</script><a href=/{path}>x</a>",
        "must be quoted",
    );
    // Expression + expression.
    assert_unsupported(
        "<script>let a = $state('x');\n\tlet b = $state('y');</script><div data-x={a}{b}></div>",
        "must be quoted",
    );
    // Expression + text.
    assert_unsupported(
        "<script>let a = $state('x');</script><img src={a}.png />",
        "must be quoted",
    );
    // ⚠️ Unlike the name/event-handler rules this is NOT element-only:
    // `validate_attribute` is called from the component visitor too
    // (`shared/component.js:93`).
    assert_unsupported(
        "<script>import F from './F.svelte';\n\tlet b = $state('y');</script><F x=a{b} />",
        "must be quoted",
    );
    // `<svelte:element>` shares `validate_element`, so it is in scope too.
    assert_unsupported(
        "<svelte:element this=\"a\" href=/{path}>x</svelte:element>",
        "must be quoted",
    );
}

#[test]
fn compile_accepts_quoted_and_single_chunk_attribute_values() {
    // ⚠️ The over-refusal half. All live-verified oracle-ACCEPTED (the two probe
    // controls reached parity).

    // The SAME multi-chunk value, quoted — the closing quote separates the last
    // chunk's end from the attribute's end, which is the whole discriminator.
    let _ = compile_js("<script>let path = $state('a');</script><a href=\"/{path}\">x</a>");
    // A single-expression value is exempt however it is delimited — the oracle's
    // `length === 1` early return ("unless the value only contains the
    // expression").
    let _ = compile_js("<script>let path = $state('a');</script><a href={path}>x</a>");
    // A single-TEXT unquoted value is exempt the same way.
    let _ = compile_js("<a href=/about>x</a>");
}

#[test]
fn compile_orders_attribute_rules_per_attribute_like_the_oracle() {
    // ⚠️ The oracle's `validate_element` is ONE loop over attributes, aborting on
    // the first error — so on `<p foo={x, y} 3aa="1">` attribute 1's sequence
    // error fires before attribute 2's name is ever inspected. A whole-list
    // pre-pass for any single rule reorders that. The refusal REASON is the
    // observable (the corpus runner buckets by it), so the interleaving is pinned.
    assert_unsupported("<p foo={x, y} 3aa=\"1\">x</p>", "sequence expression");
    // And within one attribute, the unquoted-sequence rule (`validate_attribute`)
    // runs before the sequence scan.
    assert_unsupported(
        "<script>let a = $state(1);</script><p onx={a}{a}>x</p>",
        "must be quoted",
    );
}

#[test]
fn compile_refuses_an_unparenthesized_sequence_expression_attribute() {
    // The oracle's `attribute_invalid_sequence_expression`
    // (`2-analyze/visitors/shared/element.js:52`, `shared/component.js:174`). It
    // scans the source BACKWARD from the sequence's start: `{` first means bare,
    // `(` first means parenthesized. Every shape live-verified oracle-REJECTED.

    assert_unsupported("<span foo={x, y, z} />", "sequence expression");
    // Whitespace between the delimiter and the sequence does not rescue it.
    assert_unsupported("<span foo={ x, y } />", "sequence expression");
    // A quoted single-expression value is still a single expression.
    assert_unsupported("<span foo=\"{x, y}\" />", "sequence expression");
    // ⚠️ Parenthesizing the FIRST ELEMENT is not parenthesizing the sequence: the
    // node starts at that `(`, so the scan steps past it and still finds `{`.
    assert_unsupported("<span foo={(x), y} />", "sequence expression");
    // ⚠️ Unlike the two rules above, this one is NOT element-only — a component
    // reaches it through its own visitor.
    assert_unsupported(
        "<script>import F from './F.svelte';</script><F foo={x, y} />",
        "sequence expression",
    );
    // ⭐ And the component half additionally covers `{@attach}`, which the element
    // half does not — see the accepting counterpart below. Both live-probed.
    assert_unsupported(
        "<script>import F from './F.svelte';</script><F {@attach a, b} />",
        "sequence expression",
    );
}

#[test]
fn compile_accepts_parenthesized_sequence_expressions() {
    // ⚠️ The over-refusal half — a sequence is perfectly legal when parenthesized,
    // and refusing these would reject ordinary code. All oracle-ACCEPTED.

    let _ = compile_js("<script>let x, y, z;</script><span foo={(x, y, z)} />");
    let _ = compile_js("<script>let x, y;</script><span foo={((x, y))} />");
    // A NESTED sequence: it starts at `y`, so the scan finds its own `(` at once.
    let _ = compile_js("<script>let x, y, z;</script><span foo={[x, (y, z)]} />");
    let _ = compile_js("<script>let f, x, y;</script><span foo={f((x, y))} />");
    // Only the TOP-level attribute expression is tested, so a sequence nested in a
    // larger expression is never reached regardless of parens.
    let _ = compile_js("<script>let a, b, c, d;</script><span foo={a ? (b, c) : d} />");
    // ⭐ The element/component asymmetry, in the direction that ACCEPTS: the element
    // visitor has no `{@attach}` sequence check, so this compiles where the
    // component form above refuses. Collapsing the two sites breaks this.
    let _ = compile_js("<script>let a, b;</script><span {@attach a, b} />");
}

#[test]
fn compile_refuses_a_misplaced_slot_attribute() {
    // The oracle's `slot_attribute_invalid_placement`
    // (`2-analyze/visitors/shared/attribute.js:90,123`). Its `owner` is the
    // INNERMOST ancestor that is a component / `<svelte:element>` / custom element,
    // and a slot attribute is legal only when that owner is the element's DIRECT
    // parent. Every shape below is live-verified oracle-REJECTED.

    // No owner at all.
    assert_unsupported("<div slot=\"foo\">x</div>", "slot");
    // ⚠️ Self does NOT count as its own owner — the oracle's walk is over ancestors
    // only, so a custom element carrying `slot` at the root is still misplaced.
    assert_unsupported("<custom-el slot=\"foo\">x</custom-el>", "slot");

    // Owner found, but not the direct parent — a block, an element, or a
    // `<svelte:boundary>` in between. The boundary case is why the path must carry
    // EVERY node: it is transparent to the owner search yet is still the parent.
    assert_unsupported(
        &format!("{SLOT_IMPORT}<N>{{#if t}}<div slot=\"foo\">x</div>{{/if}}</N>"),
        "slot",
    );
    assert_unsupported(
        &format!("{SLOT_IMPORT}<N><div><div slot=\"foo\">x</div></div></N>"),
        "slot",
    );
    assert_unsupported(
        &format!(
            "{SLOT_IMPORT}<N><svelte:boundary><div slot=\"foo\">x</div></svelte:boundary></N>"
        ),
        "slot",
    );
}

#[test]
fn compile_accepts_slot_attributes_the_oracle_allows() {
    // ⚠️ The over-refusal guard. All live-verified oracle-ACCEPTED.

    // An owner that is NOT a component takes neither error branch: the rule fires
    // only when the owner is a Component / `<svelte:component>` / `<svelte:self>`.
    let _ = compile_js("<custom-el><div slot=\"foo\">x</div></custom-el>");
    let _ = compile_js("<svelte:element this=\"div\"><div slot=\"foo\">x</div></svelte:element>");
    // A `{#snippet}` parent is the oracle's early return — a TEXT slot value is fine
    // there. (A non-text one is its own rule, `slot_attribute_invalid`, not ported.)
    let _ = compile_js(&format!(
        "{SLOT_IMPORT}<N>{{#snippet s()}}<div slot=\"foo\">x</div>{{/snippet}}</N>"
    ));
}

#[test]
fn compile_keeps_the_direct_child_named_slot_a_fence_not_a_placement_refusal() {
    // ⭐ METRIC-PROTECTING. `<N><div slot="foo">` — owner IS the direct parent, so
    // the oracle ACCEPTS it and tsv declines it as the deliberate runes-only
    // `ComponentNamedSlot` FENCE. If the placement rule widened to cover this shape,
    // the file would move from `fenced` to an ordinary refusal — shrinking the
    // fenced count and SILENTLY RAISING the reported achievable-parity rate with no
    // behavior change. The refusal must stay the fence.
    let source = format!("{SLOT_IMPORT}<N><div slot=\"foo\">x</div></N>");
    match compile(&source, &CompileOptions::default()) {
        Err(CompileError::Unsupported(reason)) => {
            assert!(
                reason.is_deliberate_fence(),
                "a direct-child named slot must stay a FENCE, got: {reason}"
            );
            assert!(
                reason.to_string().contains("named slot"),
                "expected the ComponentNamedSlot fence, got: {reason}"
            );
        }
        other => panic!("expected the named-slot fence, got {other:?}"),
    }
}
