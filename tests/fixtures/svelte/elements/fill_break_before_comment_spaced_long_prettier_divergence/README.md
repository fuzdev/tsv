# fill_break_before_comment_spaced_long_prettier_divergence

An HTML comment spaced on **both** sides inside a text fill is its own fill item, so the
whitespace boundary in **front** of it is a break point: that boundary is inter-node whitespace
and collapses to one rendered space at compile, so turning it into a line break is
render-equivalent. Two things follow — the word before the comment is measured **without** it, so
it keeps its column, and the comment starts a fresh line as soon as it no longer fits.

The siblings vary the other boundary. `fill_after_comment_spaced_long_prettier_divergence` is the
same width question with the whitespace *after* the comment, where the break lands in front of the
following run instead. `fill_after_comment_glued_midline_long_prettier_divergence` is the case
where the comment is glued to that following run: the glued unit admits no break inside it, so the
only break it has is this one, in front of the comment — the same boundary this fixture measures
with the glue gone.

Cases (in order):

1. **The comment ends the line at exactly 100** — it stays (control; both formatters identical).
2. **101** — one character wider, so the comment moves to its own line and the run flows after it
   there. The word before the comment stays where it is: the break taken is the one in front of
   the comment, not the one in front of the word.
3. **The comment cannot share a line with the preceding word at any column** — a 60-char word at
   line start (62) and a 51-char comment (53) each sit well within printWidth, but welded they are
   114. The break in front of the comment is the only layout that meets the limit.
4. **Nothing follows the comment** — same boundary, same break. What comes after the comment
   cannot move the break in front of it, and here there is nothing at all.
5. **Case 3 inside an INLINE container** (`<span>`) — an inline container's content is
   collapsible, so it carries no multiline cause even when width forces the break. The boundary
   does not care: same break, same widths.
6. **Case 3 inside a table cell** — inline-classified too, and at a deeper indent, so the columns
   differ while the boundary rule does not.

⚠️ Cases 5 and 6 are the container-class control, and they are not redundant with 1–4. The
question here is per-**boundary**, but the nearby machinery is keyed on the *container's*
`MultilineCause` — which is `None` for exactly these two shapes. A rule that reads that cause
looks correct on every block-container case and silently skips these; worse, the skip leaves a
stranded leading space at the head of the continuation line, which pass 2 repairs (pass 1's output
is newline-authored, hence multiline), so the damage shows up as a **lost fixed point** rather
than as a bad layout.

tsv: as above, every line ≤100.

Prettier: stable on `input`, so there is no `output_prettier.svelte` — the divergence is one of
**convergence**. Given the same document authored on one line
(`unformatted_ours_compact.svelte`), tsv reaches `input` while prettier packs the comment onto the
text line and lets the line run to 106 / 101 / 114 / 101 / 114 / 118, keeping that form.

⚠️ The compact authoring is the pin, and it has to be. Prettier's packed form
(`divergent_variant_packed.svelte`, which prettier keeps) puts a **newline** after the comment on
every case it breaks, and a newline beside a comment is authorship tsv preserves at any width —
the comment exclusion from the sibling-newline flow rule, held on each side independently
([inline_separator_comment_newline](../inline_separator_comment_newline/) pins all four
authorings, its null control included; prettier holds them too, so it is not a divergence). So tsv
rewrites the packed form to a *second* fixed point — identical to `input` except that the run after
the comment keeps its own line — and only the one-line authoring isolates the boundary this fixture
is about. Feeding the packed form back would test the comment exclusion instead.

⚠️ **That preservation is comment-specific — do not restate it as a general separator-newline
rule.** An inline element or a tag in the same position *does* flow (`text1 <span>a</span>⏎text2`
collapses), so the discriminating control swaps the comment for an element **at this same site**;
it is already built, as
[inline_sibling_newline_flow](../inline_sibling_newline_flow_prettier_divergence/)'s
`prettier_variant_newline_after` / `prettier_variant_newline_before`, which converge on that
fixture's input across all four sibling kinds. A control run where the flow rule's *other* gates
already suppress it — a run holding no prose, or an element that went multiline because of these
very newlines — keeps its lines for either sibling kind, and so "confirms" the wrong conclusion.

## Reason

Print width as a hard limit, over a boundary that is render-free.

⚠️ **Case 3 is the hard-limit claim outright, not the weaker "spend the break you have" one** that
`fill_after_comment_glued_midline_long_prettier_divergence`'s README carves out for its own case
3. There the comment alone exceeds printWidth, so no break could satisfy the limit and tsv spends
the break it has while still overrunning. Here a fully-fitting layout exists — the word (62) and
the comment (53) each fit on their own line — so leaving that boundary unused is a
[conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy)
violation rather than a tolerated overrun. Do not read the two cases as one rule.

⚠️ **The oracle cannot settle case 3.** Prettier's packed form there *is* the 114-column line, so
the limit being enforced is tsv's own and not a difference prettier could show. Neither would the
standing audits distinguish the two layouts: both are idempotent, both keep every comment, and
both reparse — only a width measurement separates them.

The boundary itself is the one
[conformance_prettier_svelte.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier_svelte.md#svelte-inline-content-block-style)
already licenses for the mirror side: inter-node whitespace collapses to one rendered space, so
tsv is free to spend it on the break. What it must not do is break there *and* re-emit the space,
which strands a leading space at the head of the continuation line — a form the next pass reads as
indentation and drops, leaving the format with no fixed point.
