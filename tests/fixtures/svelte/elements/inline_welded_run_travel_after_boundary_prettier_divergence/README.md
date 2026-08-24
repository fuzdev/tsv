# inline_welded_run_travel_after_boundary_prettier_divergence

The welded-run travel rule asked at a boundary the **fill itself owns**. Every case in
[inline_welded_run_travel_long](../inline_welded_run_travel_long_prettier_divergence/) and
[inline_welded_run_after_sibling_long](../inline_welded_run_after_sibling_long_prettier_divergence/)
varies what the run is made of and what precedes it, and each measures a boundary decided by
**width**. Here the follower is *already multiline* — a component whose glued content
(`--value_{expr1}`) has nothing to reflow, so its authored boundary newlines stand — and the
question is not whether the run fits but whether a run welded to a **forced-break** follower may
sit at the end of the previous line at all. It may not: the run travels to a fresh line, so the
component's opening tag never opens mid-line.

## Why the shape looks like it "should just collapse"

Two properties of the shape are load-bearing, and both invite the same question — *the whole
thing fits on one line, so why is it multiline at all?*

The component is multiline because the **author wrote it that way** and its content is glued
(`--value_{expr1}` is a text welded to an expression tag, so there is no whitespace seam to
reflow at). For such content the authored boundary newlines are the only structure the author
has, so both formatters keep them — the Tier-2 rule
[inline_multiline_nontext](../inline_multiline_nontext/) pins, where prettier agrees. The
**flat control** is the same document authored on one line, and both formatters keep *that*
flat: the collapse is available, it is simply the author's to make. So the multiline cases are
authored structure rather than a formatter refusing to collapse.

And the run **fits**. That is not incidental either — it is what isolates the rule. A run too
wide to fit is broken by the ordinary width measurement and travels with or without the
forced-break rule this fixture is about, so a wide shape here would pass even with the rule
removed. Only a run that fits can distinguish them.

## What varies

What the fixture pins is that **what precedes the run cannot change the answer**. The run is
byte-identical in all four multiline cases; only its predecessor varies:

- **tag predecessor** (`{expr1} =`) — the fill leads with its own `line`.
- **breaking-element predecessor** (`/> =`) — the element carries its own hard break, so the
  boundary falls to the text's fill exactly as it does after a tag. The element's attribute wrap
  answers the element's own layout, never this boundary.
- **text predecessor** (`text1 =`) — the control, which has always travelled: its fill leads with
  a word rather than a `line`.
- **spaced run after a tag** (`{expr1} text1` + `<Comp …>`) — the second control, the same
  predecessor as case 1 with the weld removed. It has always travelled too.

The two controls are what make this a rule rather than a special case: they bracket the two axes
(predecessor kind, welded-or-spaced) on which the answer must not differ. A run's fill leads with a
`line` exactly when its predecessor is a tag or an element, which shifts the fill's
content/separator parity by one — and the forced-break measurement was reached only on the
word-leading parity, so `text1 var(<Comp …>` travelled while `{expr1} var(<Comp …>` welded and
opened the tag mid-line. One rule, two answers, keyed on a predecessor the rule is not about.

The boundary tsv spends is inter-node whitespace that renders as one space either way, so every
output here renders identically to the input.

⚠️ A regression is invisible to every idempotency-shaped gate: both the travelled and the
mid-line forms are their own fixed points, and neither is over-width, so F1, the seeded fuzzer,
`authoring:audit`, the round-trip and the width ratchet all pass straight through. `input.svelte`
failing to be a tsv fixed point is what catches it.

## Prettier's form

`output_prettier.svelte` keeps the run welded to the previous line in **all four** multiline
cases, including the text-predecessor control — prettier's boundary measurement stops at the
follower's first internal break, reports a fit, and opens the component's tag mid-line. So the
divergence is uniform, and the controls are controls for tsv's own consistency rather than for
prettier's. The **flat control is untouched by prettier too**, which is what makes it a control
rather than a fifth divergence: both formatters preserve the authoring in each direction.

## Reason

Design choice: "unbreakable inside" is not "immovable" — a welded unit spends the render-free
boundary in front of it rather than standing and opening a forced-break follower's tag at the end
of the previous line. tsv lays wrapping inline content out block-style where prettier dangles or
hugs the tag delimiters.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style).
