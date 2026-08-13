# inline_separator_tab_prettier_divergence

tsv: a **tab** separating two non-text siblings makes the content a reflowable fill exactly as a
space does, so both authorings converge on the one fully inline form. Prettier: keeps a distinct
stable form per spelling — and, for the boundary-newline authoring, per sibling kind as well.

## Reason

**Design choice — rule 1 of Svelte 5's whitespace model.** An inter-sibling whitespace run
collapses to a single whitespace, so a separator's *presence* carries signal while its *spelling*
carries none: space and tab render identically and are one document. Wherever tsv lets the spelling
pick a layout instead, that is a bug against rule 1 rather than a divergence from prettier —
prettier converges neither axis, so it is no oracle here and the bar is tsv-vs-tsv consistency. See
[§Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).

⚠️ **Two of the three cases get "renders identically" from the compiler; the tag pair gets it from
the browser.** `clean_nodes` skips the collapse whenever a neighbor is an `ExpressionTag` ("in the
end they count as one text"), so `<code>a</code>⇥<code>b</code>` and `<Comp />⇥<Comp />` compile to
a single space while `{a}⇥{b}` reaches compiled output **with its tab intact**. All three still
render alike under `white-space: normal` — the browser collapses the run itself, and that is the
model tsv's render-equivalence oracle implements — so the convergence target is unchanged and the
render-equivalence check passes on the merits, not by accident. But the tag case is not a rule-1
consequence and must not be restated as one; under `white-space: pre`/`pre-wrap` those two
authorings do differ. tsv already takes this position wherever a spaced tag pair goes block-style
and each tag drops to its own line.

The separator is what makes the content a **fill to reflow into**, which is in turn what makes the
render-free content boundary stop selecting the layout. That question is asked of the run, so it
must not be asked of the separator's spelling: a space-separated sibling pair already collapsed,
and the tab-separated twin has to reach the same place.

**The newline is deliberately not part of this equivalence**, and that asymmetry is the point of
the closing sentence in `input.svelte`'s comment rather than an oversight. A run of pure siblings
holds no prose, so it has no fill to reflow into; its authored lines are the only structure the
author has, and collapsing them packs independent siblings onto one line. So the newline spelling
keeps its lines — pinned next door by [inline_multiline_nontext](../inline_multiline_nontext/),
where prettier agrees. What separates the tab from the newline is not the byte but whether it is a
**break**: a tab leaves the siblings on one line, so there is no authored break for the interior
arms to honor, while a newline is one and they honor it. No fill answer is involved on either
side — a pure-sibling run has none, and the arm that once granted it one is retired as
decision-inert.

## Cases

The three non-text sibling kinds — an expression-tag pair, an element pair, and a component pair —
each in the converged inline form, with two authorings:

- `unformatted_tab.svelte` — the tab spelling with the boundaries already hugged. **Both**
  formatters normalize this to `input`, so it is the control that isolates the defect: the tab
  alone was never the problem.
- `variant_boundary_newline_tab.svelte` — the same document with the content boundaries
  newline-authored. **Dual-stable**: the authored air is preserved by both formatters (see
  [inline_boundary_air](../inline_boundary_air/), where the tab spelling of that form is pinned
  as an `unformatted_*`), and its separators are spelled as spaces, which is the tab rule this
  fixture is about applied inside the preserved air.

`variant_*` rather than `prettier_variant_*` is the load-bearing part of that pin: the form is
stable under **both** formatters, so the boundary axis carries no divergence at all. Its
separators are spelled as spaces on every sibling kind, and a pure-sibling run authored across
lines keeps its lines (the asymmetry above), so reading it next to `input` shows the frontier
directly: tsv converges the tab with the space and holds the newline apart, on all three sibling
kinds, whether or not the content boundaries carry air.

Together they locate the rule-1 violation precisely: the spelling only decided the layout when the
boundary was newline-authored, because that is the only case in which the boundary had a layout to
select in the first place.

See
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
