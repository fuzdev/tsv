# fill_after_comment_glued_midline_long_prettier_divergence

A text run byte-glued to a preceding HTML comment travels **with** it: that boundary carries no
whitespace, so the only break point is the one in **front** of the comment. This fixture is the
`midline` case — the glued unit does not start its line, so that break point actually exists.

The glued sibling `fill_after_comment_glued_long` is the same shape at **line start**, where there
is no break point ahead of the unit at all and it simply stands. The spaced sibling
`fill_after_comment_spaced_long_prettier_divergence` is the same width question with the whitespace
*inside* the boundary, where the break lands after the comment instead of before it.

Cases (in order):

1. **Fits mid-line at 98** — the unit stays where it is; the run wraps normally before `text2`
   (control; both formatters identical).
2. **Overflows mid-line at 104, fits at line start** — the whole unit moves to a fresh line and
   lands at exactly 100.
3. **Wider than a whole line at 101** — the unit moves anyway. The break is spent; the limit is
   still not met, because the comment alone exceeds print width.
4. **The boundary in front of the unit is itself glued** (`0<!--…-->text1`) — so the unit cannot
   move, and the break travels further left, to the space inside the preceding run. This is the
   case that says fusing the prefix **moves** the guarded boundary rather than retiring it: the
   unit now begins at the comment, so its leading boundary is the one in front of the *comment*,
   and breaking there would inject a rendered space just as breaking inside it would. Found by the
   seeded fuzzer (`deno task fuzz:audit`) mutating cases 1–3, and reachable by nothing else here —
   the mangled form is a fixed point, so idempotency is blind to it.
5. **Mid-run after a wide word, and terminal** — the unit's leading boundary sits inside a
   multi-word fill rather than just after the run's first word, and nothing follows the unit. The
   wide word (62) and the unit (58) each fit on a line but not together, so unlike case 3 the move
   **does** meet the limit outright. The word before the unit is measured *without* it and holds
   its own line; only the unit travels. A follower after the unit is what cases 1–4 all have, and
   it is not what licenses the break — this case has none and takes the same break.

tsv: as above.

Prettier: identical on case 1. On cases 2, 3 and 5 it **welds** the unit to the preceding word and
lets the line run to 104 / 105 / 119; it keeps both the welded form and `input` stable there, so
those three are a **normalization** divergence — `prettier_variant_welded.svelte` holds the form
prettier keeps and tsv rewrites to `input`. On case 4 it does not keep `input` stable at all,
packing the run back onto one 105-column line: `output_prettier.svelte`.

## Reason

The boundary in front of the unit is inter-node whitespace, which the compiler collapses to one
rendered space, so turning it into a line break is render-equivalent — tsv is free to spend it. It
must not break there *and* re-emit the space, which would strand a leading space the next pass
reads as indentation and drops (no fixed point); the break consumes it.

⚠️ **Case 3's claim is "spend the break you have", not "meet the limit"** — it is the honest one. A
comment wider than print width cannot be made to fit by any break, and tsv moves the unit anyway
rather than leaving an available break unused. That is a *weaker* claim than
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy);
do not read case 3 as evidence that the hard-limit rule reaches shapes where no break can satisfy
it. Cases 2 and 5 satisfy it outright and are where the hard limit does the work — 5 in particular
holds it from **mid-run**, the position case 3's caveat must not be read as excusing.

The spaced mirror of case 5 — the same boundary with the glue gone from the comment's far side, so
the comment is its own fill item — is
`fill_break_before_comment_spaced_long_prettier_divergence`. The two shapes share one boundary and
one rule; only what sits on the comment's *far* side differs.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style)
for the render-free boundary rule the first paragraph rests on.
