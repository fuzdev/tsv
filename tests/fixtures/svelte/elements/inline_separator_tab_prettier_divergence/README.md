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
[§Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).

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
where prettier agrees. What separates the tab from the newline is not the byte but the fact that
the author left the siblings on **one line**, which is exactly the condition this arm reads.

## Cases

The three non-text sibling kinds — an expression-tag pair, an element pair, and a component pair —
each in the converged inline form, with two authorings:

- `unformatted_tab.svelte` — the tab spelling with the boundaries already hugged. **Both**
  formatters normalize this to `input`, so it is the control that isolates the defect: the tab
  alone was never the problem.
- `unformatted_ours_boundary_newline_tab.svelte` — the tab spelling with the content boundaries
  newline-authored. Only tsv normalizes it to `input`; prettier settles on its own stable form,
  pinned as `divergent_variant_boundary_newline_tab.svelte`.

`divergent_variant_*` rather than `prettier_variant_*` is the load-bearing part of that pin, and it
is what the divergence actually looks like: prettier's form spells the tag-pair and component-pair
separators as **newlines** — and tsv does not normalize those back to `input`, since a pure-sibling
run authored across lines keeps its lines (the asymmetry above) — while the element pair's separator
prettier had already joined onto one line. So tsv rewrites prettier's form to a *third* stable form,
collapsing only that element pair. Reading that pin
next to `input` shows the frontier directly: tsv converges the tab with the space and holds the
newline apart, on all three sibling kinds.

Together they locate the rule-1 violation precisely: the spelling only decided the layout when the
boundary was newline-authored, because that is the only case in which the boundary had a layout to
select in the first place.

See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
