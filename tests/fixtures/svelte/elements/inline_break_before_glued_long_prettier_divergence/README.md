# inline_break_before_glued_long_prettier_divergence

The break-before-an-inline-element rule (see `inline_break_before_wrap_long`) breaks at the
last **whitespace** boundary before the element — never between glued text and the element.
When an inline element is glued (no whitespace) to preceding text, that text travels *with*
the element to the fresh line. Here `glued` is glued to the `<a>`, so the break lands before
`glued` and `glued<a …>content</a>.` moves to the fresh line as one unit.

Breaking between `glued` and `<a>` would be **render-changing** — the glued boundary is
render-significant (it injects a rendered space, so the text data would gain a trailing
space). tsv only ever breaks at the whitespace boundary before the glued run, which is
render-equivalent (confirmed by `ast_diff --render`).

The second and third cases pin the measurement's **far edge** at the exact 100/101 boundary,
with a spaced non-terminal follower (` mid <b>x</b>`) after the element: the unit is measured
to its own end and no further, so the follower — and the unit's own trailing boundary, the
space in front of `mid` — never enters the unit's fit check. At exactly 100 the unit packs
onto the text line and only the follower wraps (a form **both** formatters keep — counting
the trailing boundary into the unit would break this line one column early); at 101 the unit
travels and the follower packs after it on the fresh line.

The fourth and fifth cases run the glue **through an expression tag** (`glued{x}<a …>`): a
glued tag welded onward into an element is mid-run glue like any other, so the unit travels
whole — the fourth crosses the width past the tag (inside the element), the fifth inside the
tag itself, and both reach the same form (which side of the tag the width runs out on cannot
matter). A glued tag that **ends** the run travels the same way — the word+tag pair is the
smallest welded unit; see `fill_glued_tag_travel_long_prettier_divergence` for that
contract's own boundary cases.

The sixth case glues into a tag whose **expression itself must break** (a wide ternary,
welded onward to a `)`): the unit still travels first — the flat measurement fails, so the
boundary in front breaks — and the expression then breaks internally on the fresh line. The
wide-element rule's tag analog: content that cannot fit flat starts on a fresh line rather
than opening mid-line (prettier opens the tag mid-line and breaks inside it).

The seventh and eighth cases weld the tag onward into **another tag** (`glued{x}{y}`), at
the exact 100/101 boundary: mid-run glue again — the boundary's measurement walks through
the first tag into the second — so at 100 the packed line stands (both formatters keep it)
and at 101 the whole unit travels (prettier packs it at 101).

The ninth and tenth cases glue the tag to a following **block element** at the same
100/101 boundary — pinning what is IN the measured unit. The glue survives only in the
source: the block detaches to its own line by its own layout (render-free at a block
boundary), so the weld does not survive and the measured unit is the word+tag pair alone —
were the source glue counted, the pair-plus-block measurement would break the @100 line a
member early. At exactly 100 the pair packs (both formatters keep it); at 101 it travels
(prettier packs it at 101, riding the tag past printWidth). The pair-travel itself is the
smallest-welded-unit contract — see `fill_glued_tag_travel_long_prettier_divergence`.

tsv: `glued<a>` stays glued and moves to its own line together.
Prettier: keeps the glued run on the text line and dangles the `<a>` closing tag — see
`output_prettier.svelte` (prettier's stable form). `unformatted_ours_compact.svelte` is the
compact authoring (tsv → `input.svelte`, prettier → `output_prettier.svelte`).

## Reason

Design choice, render-free under Svelte 5 for the *whitespace* boundary; render-significant
for the *glued* boundary, which is therefore never split.
See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
