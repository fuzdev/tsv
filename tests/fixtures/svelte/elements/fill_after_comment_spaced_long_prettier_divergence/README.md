# fill_after_comment_spaced_long_prettier_divergence

A text run separated from a preceding HTML comment by a **space** keeps a break point at that
boundary: inter-node whitespace collapses to one rendered space at compile, so turning it into a
line break is render-equivalent. Once the run's first word no longer fits after the comment, the
whole run starts a fresh line and flows there as one fill — the boundary space is **spent on the
break**, never re-emitted at the head of the continuation line.

The glued sibling is `fill_after_comment_glued_long`: with no whitespace at that boundary the
break point does not exist at all, and both formatters keep the run welded to the comment.

Cases (in order):

1. **Whole line fits at exactly 100** — comment, space and run stay inline (control; both
   formatters identical).
2. **First word fits at exactly 100** — it stays on the comment line and the rest wraps.
3. **101** — the first word no longer fits, so the whole run moves to its own line and flows
   there as one fill, with no leading space.
4. **Long run after the break** — the run wraps again on its fresh line, greedily and within
   printWidth.
5. **The same boundary in an INLINE container** (`<span>`) — the container-class control. An
   inline container's content is collapsible, so it carries no multiline cause even when width
   forces the break; the boundary is the same question there and takes the same break.

⚠️ Case 5 is not redundant with 1–4. The rule is per-**boundary**, but the nearby machinery is
keyed on the *container's* `MultilineCause`, which is `None` for exactly this shape. Keyed on
that cause, the boundary is skipped here and the space is baked into the run's first word
instead — which then strands at the head of the continuation line (`-->⏎\t text1`) and shatters
the rest of the run onto a third line. That output is **not idempotent**: pass 2 repairs it,
because pass 1's output is newline-authored and hence multiline. So the damage shows up as a
lost fixed point on a shape no earlier case had, not as a visibly wrong layout.

tsv: as above, every line ≤100.

Prettier: identical on cases 1, 3 and 4 — it keeps the run on its own line once the first word
no longer fits, and is stable on that form. It differs on case 2, where it packs the whole run
onto the comment line and lets it run to 106 columns; tsv treats printWidth as a hard limit and
wraps. See `output_prettier.svelte`. On case 5 it additionally **dangles** the inline container's
tag delimiters (`<span⏎\t>…</span⏎>`) — its own cataloged divergence, not this fixture's subject,
but unavoidable in any inline-container case; `prettier_variant_first_word.svelte` carries that
form, which tsv normalizes to `input`.

## Reason

Print width as a hard limit (case 2), and render-free boundary layout for the rest. The
whitespace boundary before the run collapses to a single rendered space at compile, so tsv is
free to spend it on the line break; what it must not do is break there *and* re-emit the space,
which strands a leading space at the head of the continuation line — a form the next pass reads
as indentation and drops, so the format would have no fixed point.
See [conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)
and [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
