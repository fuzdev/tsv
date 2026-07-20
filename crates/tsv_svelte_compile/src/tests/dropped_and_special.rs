//! Dropped regions and the SSR-inert special elements.

use super::support::*;

#[test]
fn dropped_fragments_are_walked() {
    // The M4 class, pinned. A fragment the emitter DISCARDS without visiting —
    // today only `{:catch}`, plus an event handler's expression — must still be
    // walked for the things the oracle decides BEFORE it chooses what to emit.
    // Dropping the region cannot make the component valid.
    //
    // This test covers the two facts carried by an EXPRESSION inside the region:
    // its reference counting (`needs_context`) and its analysis-phase errors (a
    // misplaced rune). The third fact — one the oracle reads from a node's mere
    // PRESENCE, independent of any expression — is a different walk over node
    // KINDS, pinned separately by `dropped_fragment_refuses_presence_read_nodes`.
    //
    // A new emission-dropped fragment that skips that walk fails here (and, for
    // TypeScript, in `compile_refuses_template_typescript_without_lang_ts`).

    // 1. References inside a dropped `{:catch}` still reach `needs_context`: a
    //    prop-rooted member access there forces the `$$renderer.component`
    //    wrapper, exactly as the oracle counts it.
    let js = compile_js(
        "<script>\n\tlet { p, obj } = $props();\n</script>\n\
             {#await p then v}<p>{v}</p>{:catch e}<p>{obj.field}</p>{/await}",
    );
    assert!(
        js.contains("$$renderer.component(($$renderer) => {"),
        "a prop-rooted access in a dropped {{:catch}} must still fire needs_context: {js}"
    );

    // 2. An analysis-phase error inside a dropped region still refuses — the
    //    oracle rejects `{:catch e}{$state(1)}` with `state_invalid_placement`.
    assert_unsupported(
        "<script>\n\tlet { p } = $props();\n</script>\n\
             {#await p then v}<p>{v}</p>{:catch e}<p>{$state(1)}</p>{/await}",
        "$state",
    );
    assert_unsupported(
        "<script>\n\tlet { a } = $props();\n</script>\n\
             <button onclick={() => $state(1)}>b</button>",
        "$state",
    );

    // 3. …but a shape the oracle merely DROPS must still compile: a derived read
    //    inside a dropped `{:catch}` is emitted nowhere, so the oracle accepts
    //    it. Guarding a dropped region must not over-refuse.
    let derived = compile_js(
        "<script>\n\tlet { p } = $props();\n\tlet d = $derived(1);\n</script>\n\
             {#await p then v}<p>{v}</p>{:catch e}<p>{d}</p>{/await}",
    );
    assert!(
        !derived.contains("catch"),
        "the {{:catch}} branch is dropped from SSR: {derived}"
    );
}

/// The dropped-region walk over node KINDS, and the rule that scopes it.
///
/// `guard_dropped_fragment`'s expression walk asks what a dropped region
/// *references*. It never asks what a dropped node *is* — so a fact the oracle
/// reads out of a node's mere presence reaches no guard at all. `<slot>` is that
/// fact: the oracle's phase-2 analysis records every `<slot>` in `slot_names`
/// (`2-analyze/visitors/SlotElement.js`) and phase 3 folds `slot_names.size > 0`
/// into `should_inject_props`, so a `<slot>` in a `{:catch}` widens the component
/// signature to `($$renderer, $$props)` even though SSR emits nothing from the
/// branch. Without the node walk this pins, tsv fires no refusal there and emits
/// the bare signature — a MISMATCH.
///
/// The rule is NOT "every fenced construct refuses everywhere". It is: a construct
/// refuses everywhere it can **affect the result**. A dropped region suppresses a
/// construct's emission but not a phase-2 fact keyed on its presence — and there
/// are TWO such kinds of fact, which is the part that is easy to get wrong:
///
/// - **emission** — the fact rides into the generated code (`<slot>` → the widened
///   signature, above). Measurable one construct at a time.
/// - **validation** — the fact feeds a whole-component check that can turn an
///   otherwise-valid component into a compile error (a dropped `on:` + an emitted
///   `onclick` → `mixed_event_handler_syntaxes`). Invisible in isolation: it needs
///   a SECOND construct elsewhere in the component to fire, so a per-construct
///   probe reports the dropped one as inert and is simply asking the wrong
///   question.
///
/// The third loop pins the validation axis; the second pins that everything on
/// neither axis keeps compiling, which is what makes the distinction load-bearing
/// rather than a slogan.
#[test]
fn dropped_fragment_refuses_presence_read_nodes() {
    // A `$state` promise, deliberately NOT `$props()`: the fact under test is a
    // widened component signature, so the baseline must be the bare
    // `($$renderer)` one that a `$props()` destructure would mask.
    let await_ = |catch: &str| {
        format!(
            "<script>\n\tlet p = $state(Promise.resolve(1));\n</script>\n\
             {{#await p}}a{{:then v}}b{{:catch e}}{catch}{{/await}}"
        )
    };

    // ── refuses: `<slot>`, the one presence-read construct ────────────────
    //
    // Same bucket label as the emitted path (`template node special element
    // <slot>`), so the refusal is the fence firing in a second position rather
    // than a new reason — and the corpus census keeps one bucket for the tag.
    for catch in [
        "<slot />",
        // Nested under an element: the walk must recurse into child fragments.
        "<div><slot /></div>",
        // Named — `slot_names` keys by name, but any key makes the map non-empty.
        "<slot name=\"x\" />",
        // Nested under a BLOCK inside the dropped branch: recursion has to ride
        // the structural seam, not just element children.
        "{#if e}<slot />{/if}",
        "{#each [e] as x}<slot />{/each}",
        // The other container kinds `each_child_fragment` enumerates — a
        // component's children, a `{#snippet}` body, a nested `{#await}`, and a
        // special element's own fragment. Each is a distinct arm of that match.
        "<Foo><slot /></Foo>",
        "{#snippet s()}<slot />{/snippet}",
        "{#await p}<slot />{:then q}x{/await}",
        "<svelte:boundary><slot /></svelte:boundary>",
    ] {
        assert_unsupported(&await_(catch), "template node special element <slot>");
    }

    // ── refuses: the legacy directives, on the VALIDATION axis ────────────
    //
    // A dropped `on:` is not inert. `2-analyze/visitors/OnDirective.js` sets
    // `analysis.event_directive_node` wherever the directive sits, an
    // `onclick`-style attribute on an EMITTED element sets
    // `analysis.uses_event_attributes` (`visitors/Attribute.js`), and
    // `2-analyze/index.js` errors `mixed_event_handler_syntaxes` when both are
    // set. So `{:catch}<button on:click=…>` plus a sibling `<div onclick=…>`
    // makes the oracle REJECT the component while tsv compiled it — an
    // over-acceptance, which this pins closed. It refuses unconditionally rather
    // than only in the mixed configuration: the sibling is arbitrarily far away
    // and the construct is a permanent fence, so there is nothing to buy by
    // being clever about it.
    //
    // `let:` rides along on the fence argument, NOT this one — its only oracle
    // check is the local `let_directive_invalid_placement`, which reads its
    // parent and writes no whole-component field. It is genuinely inert here and
    // refuses anyway, to keep the fenced pair in one census bucket.
    for (catch, refusal) in [
        ("<button on:click={() => e}>x</button>", "on:"),
        ("<div let:x>y</div>", "let:"),
        // Nested, and on a component / special element — the directive hangs off
        // an attribute list, so the walk has to reach every host that has one.
        ("<div><button on:click={() => e}>x</button></div>", "on:"),
        ("<Foo on:click={() => e} />", "on:"),
        ("<Foo let:x>y</Foo>", "let:"),
        ("{#if e}<button on:click={() => e}>x</button>{/if}", "on:"),
        (
            "<svelte:boundary><i on:click={() => e}>x</i></svelte:boundary>",
            "on:",
        ),
    ] {
        assert_unsupported(
            &await_(catch),
            &format!("legacy {refusal} directive (runes-only fence)"),
        );
    }

    // The full mixed configuration, verbatim — the shape the isolated probes
    // report as inert and the oracle rejects. `await_` cannot express it (the
    // sibling sits OUTSIDE the block), so it is spelled out.
    assert_unsupported(
        "<script>\n\tlet p = $state(Promise.resolve(1));\n</script>\n\
         {#await p}a{:then v}b{:catch e}<button on:click={() => e}>x</button>{/await}\n\
         <div onclick={() => p}>y</div>",
        "legacy on: directive (runes-only fence)",
    );

    // ── must NOT over-refuse: everything else at the same position ────────
    //
    // Each of these is emission-refused in an EMITTED position, and each reaches
    // parity with the oracle inside `{:catch}` today. Refusing them to satisfy a
    // uniform "fenced refuses everywhere" rule would trade correct output for
    // nothing.
    //
    // This set is clean on BOTH axes, and the validation half is verified by
    // reading the writers rather than by probing: the whole-component fields a
    // phase-2 validation reads are `slot_names`, `uses_slots`, `uses_render_tags`,
    // `event_directive_node`, `uses_event_attributes`, and `snippets` — written
    // ONLY by `SlotElement` / an `$$slots` `Identifier` / `RenderTag` /
    // `OnDirective` / an event `Attribute` / `SnippetBlock`. No construct below
    // writes any of them, so no combination can make one of them matter.
    // (`<svelte:boundary>` is not even fenced — a first-class Svelte 5 feature and
    // the next implementation target — so refusing it would actively obstruct
    // that work.)
    for catch in [
        "<svelte:boundary><i>x</i></svelte:boundary>",
        "<svelte:component this={e} />",
        "<svelte:element this=\"div\" />",
        // `<svelte:self>` only in a nesting the oracle permits — bare inside
        // `{:catch}` it is `svelte_self_invalid_placement`, so unreachable.
        "{#if e}<svelte:self />{/if}",
        // `<svelte:fragment>` and a `slot=\"…\"` child likewise want a component
        // parent; the named-slot fence is the CONSUMER half of the slot system
        // whose declaring half (`<slot>`) refuses two loops above.
        "<Foo><svelte:fragment slot=\"x\">y</svelte:fragment></Foo>",
        "<Foo><p slot=\"x\">y</p></Foo>",
    ] {
        let js = compile_js(&await_(catch));
        assert!(
            js.contains("export default function Input($$renderer) {"),
            "a dropped `{{:catch}}` {catch} must compile with the bare signature: {js}"
        );
    }
}

#[test]
fn compile_ssr_inert_special_elements() {
    // `<svelte:window>`/`<svelte:body>`/`<svelte:document>` are SSR-inert: their
    // events/binds are client-only, so the oracle emits NOTHING for them. A bare
    // one leaves only the empty exported function.
    assert_eq!(
        compile_js("<svelte:window />"),
        "import * as $ from 'svelte/internal/server';\n\
             export default function Input($$renderer) {}\n"
    );
    // Beside real content: the content still emits, the window drops (its only
    // template output is the sibling's push — no window marker).
    let beside = compile_js("<svelte:window />\n<p>real</p>");
    assert!(
        beside.contains("$$renderer.push(`<p>real</p>`)") && !beside.contains("svelte:window"),
        "window drops, sibling content emits: {beside}"
    );
    // The attribute expressions are still WALKED by needs_context: a prop-rooted
    // member in a window handler fires the `$$renderer.component` wrapper, exactly
    // as the oracle counts it.
    let wrapped = compile_js(
        "<script>\n\tlet { p } = $props();\n</script>\n<svelte:window onkeydown={p.method} />",
    );
    assert!(
        wrapped.contains("$$renderer.component(($$renderer) => {"),
        "a prop-rooted access in a window handler must fire needs_context: {wrapped}"
    );
    // A `bind:` marks its target reassigned, so a later `{y}` read stays dynamic
    // (not folded to its initial value).
    let bound = compile_js(
        "<script>\n\tlet y = $state(0);\n</script>\n<svelte:window bind:scrollY={y} />{y}",
    );
    assert!(
        bound.contains("$.escape(y)"),
        "a window bind must keep a later read dynamic (not folded): {bound}"
    );
    // A stray rune inside a window attribute still refuses (the oracle rejects it
    // as `state_invalid_placement` at analysis).
    assert_unsupported("<svelte:window onkeydown={$state(0)} />", "$state");

    // A valid mix of MODERN-runes attributes compiles to nothing: a modern event
    // attribute (guard-dropped), two whitelisted binds (`focused`/`innerWidth`),
    // and a `class:` directive — all oracle-accepted, all dropped from SSR output,
    // so the body is just the (rewritten) script declarations, no window markup.
    let combined = compile_js(
        "<script>\n\tlet f = $state(0);\n\tlet w = $state(0);\n\tlet x = $state(false);\n</script>\n\
         <svelte:window onclick={() => {}} bind:focused={f} bind:innerWidth={w} class:c={x} />",
    );
    assert!(
        !combined.contains("$$renderer.push") && !combined.contains("svelte:window"),
        "a valid inert element with modern attrs compiles to nothing: {combined}"
    );

    // A whitelisted bind with a VALID target compiles (dropped): a `$state`-rooted
    // lvalue for a normal bind (`innerWidth`), and ANY lvalue for `bind:this` (no
    // `$state` gate — even an uninitialized `let el`), matching the regular-element
    // fork.
    // a whitelisted bind with a valid $state / bind:this target must compile
    let _ = compile_js(
        "<script>\n\tlet s = $state(0);\n\tlet el;\n</script>\n\
         <svelte:window bind:innerWidth={s} bind:this={el} />",
    );

    // The no-op drop family is oracle-accepted on these elements and guard-dropped
    // (`class:`/`style:`/`use:`/`transition:`/`in:`/`out:`/`animate:`/`{@attach}`).
    for attr in [
        "class:c={ok}",
        "style:color={ok}",
        "use:ok",
        "transition:ok",
        "in:ok",
        "out:ok",
        "animate:ok",
        "{@attach ok}",
    ] {
        // drop-family directive must compile on an inert element
        let src = format!("<script>\n\tlet ok = 0;\n</script>\n<svelte:window {attr} />");
        let _ = compile_js(&src);
    }
}

#[test]
fn compile_refuses_invalid_ssr_inert_special_elements() {
    // Invalid-input shapes the oracle rejects at analysis; tsv's parser accepts
    // them, so the compiler must refuse (never emit nothing for oracle-rejected
    // input, which would surface as a corpus OVER-ACCEPTANCE).
    //
    // PLACEMENT: legal only at the component root — nested inside an element/block/
    // snippet is `svelte_meta_invalid_placement`.
    assert_unsupported(
        "<div><svelte:window onkeydown={() => {}} /></div>",
        "must be a top-level element",
    );
    assert_unsupported(
        "{#if true}<svelte:body use:act />{/if}",
        "must be a top-level element",
    );
    // DUPLICATE: at most one of each kind (`svelte_meta_duplicate`).
    assert_unsupported(
        "<svelte:window /><svelte:window />",
        "duplicate <svelte:window> element",
    );
    // Different kinds side-by-side are fine (not a duplicate).
    let _ = compile_js("<svelte:window /><svelte:body />");

    // CHILDREN: `disallow_children` — these cannot have children
    // (`svelte_meta_invalid_content`). tsv's parser DOES parse them into the
    // fragment, so refuse.
    assert_unsupported("<svelte:window>hi</svelte:window>", "cannot have children");
    assert_unsupported(
        "<svelte:body><p>x</p></svelte:body>",
        "cannot have children",
    );

    // ILLEGAL ATTRIBUTE: only a modern event attribute (`on*={expr}`) is legal; a
    // non-event plain attribute, a string-valued handler, a bare handler, and a
    // spread refuse (`illegal_element_attribute` / `svelte_body_illegal_attribute`).
    assert_unsupported("<svelte:window class=\"x\" />", "invalid attribute");
    assert_unsupported("<svelte:window id={x} />", "invalid attribute");
    assert_unsupported("<svelte:window onkeydown=\"str\" />", "invalid attribute");
    assert_unsupported("<svelte:window onclick />", "invalid attribute");
    assert_unsupported("<svelte:window {...o} />", "invalid attribute");

    // INVALID BIND: a name outside the per-kind whitelist refuses. `bind:scrollY`
    // is window-only (`bind_invalid_target` on body); `bind:nonexistent` is not a
    // binding (`bind_invalid_name`); `bind:clientWidth` is invalid on window
    // (`bind_invalid_name`). Valid $state target isolates the NAME check.
    assert_unsupported(
        "<script>\n\tlet y = $state(0);\n</script>\n<svelte:body bind:scrollY={y} />",
        "bind: directive scrollY",
    );
    assert_unsupported(
        "<script>\n\tlet a = $state(0);\n</script>\n<svelte:window bind:nonexistent={a} />",
        "bind: directive nonexistent",
    );
    assert_unsupported(
        "<script>\n\tlet a = $state(0);\n</script>\n<svelte:window bind:clientWidth={a} />",
        "bind: directive clientWidth",
    );

    // INVALID BIND TARGET: a whitelisted NAME with a target that is not a
    // `$state`-rooted lvalue refuses — the SAME reassignable-lvalue rule regular
    // elements enforce (`validate_inert_bind_target`). A non-lvalue (call / literal),
    // a `const`, and an undefined identifier the oracle also rejects
    // (`bind_invalid_expression` / `constant_binding` / `bind_invalid_value`) — this
    // closes the target over-acceptance the blanket guard-drop had left open.
    assert_unsupported(
        "<script>\n\tlet s = $state(0);\n</script>\n<svelte:window bind:innerWidth={foo()} />",
        "bind: directive innerWidth",
    );
    assert_unsupported(
        "<svelte:window bind:innerWidth={5} />",
        "bind: directive innerWidth",
    );
    // A `const` target now refuses one step EARLIER, in the whole-component
    // `validate_assignment` port (which reaches every `bind:` the oracle's own
    // validator does, dropped regions included) — so it carries the sharper
    // `constant_assignment` bucket rather than the bind-shaped one. The
    // reassignable-lvalue rule in the bind path is unchanged and still stands
    // behind it.
    assert_unsupported(
        "<script>\n\tconst c = 1;\n</script>\n<svelte:window bind:innerWidth={c} />",
        "a constant",
    );
    assert_unsupported(
        "<svelte:window bind:innerWidth={undefinedVar} />",
        "bind: directive innerWidth",
    );

    // LEGACY DIRECTIVES: a legacy `on:` event directive and `let:` refuse
    // (`RunesOnlyFence`) — the runes-only fence, matching the regular-element
    // path. The oracle ACCEPTS `on:` here, so this is a deliberate safe
    // over-refusal, not an oracle-parity claim.
    assert_unsupported(
        "<svelte:window on:click={() => {}} />",
        "legacy on: directive (runes-only fence)",
    );
    assert_unsupported(
        "<svelte:body let:x />",
        "legacy let: directive (runes-only fence)",
    );
}

/// Pin every refused special-element kind to its OWN bucket label.
///
/// `special_element_kind_table!` expands one row set into the mapping and
/// `SPECIAL_ELEMENT_REFUSAL_KINDS` together, so a NEW kind cannot reach either
/// without the other, and pairing each pattern with its label in the row rules out
/// an index that a reorder could re-point. What none of that catches is a row
/// written with the WRONG label — which would silently relabel one tag as another.
/// That matters beyond cosmetics: `Refusal::is_deliberate_fence` keys the runes-only
/// fence set on these labels, so a swap would move a tag in or out of the
/// achievable-parity denominator.
#[test]
fn refused_special_elements_carry_their_own_bucket_label() {
    for (source, tag) in [
        (
            "<script>\n\tconst Foo = null;\n</script>\n<svelte:component this={Foo} />",
            "<svelte:component>",
        ),
        ("<svelte:self />", "<svelte:self>"),
        ("<slot />", "<slot>"),
        ("<svelte:fragment />", "<svelte:fragment>"),
        // `<svelte:boundary>` is deliberately absent — it COMPILES, so it carries
        // no `TemplateNode` label at all.
    ] {
        assert_unsupported(source, &format!("template node special element {tag}"));
    }
}
