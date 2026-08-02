# fill_glued_tag_travel_long_prettier_divergence

A `{expr}` / `{@html}` / `{@render}` tag glued to the end of a text word is the **smallest
welded unit**: the word and its tag share one fit check, and when the pair does not fit, the
break lands at the last whitespace boundary before the word — the pair moves to the fresh
line together, holding printWidth as a hard limit. Breaking between the word and the tag is
never an option (the glued boundary is render-significant — a break there would inject a
rendered space); the whitespace boundary in front of the word is inter-node whitespace that
collapses at compile, so spending it is render-free.

Prettier keeps the tag *outside* the text fill, so its fill never sees the tag's width: the
word stays put and the tag rides past printWidth after it — see `output_prettier.svelte`.
tsv treats the pair like any other welded unit (the same travel rule as
`inline_break_before_glued_long` and `inline_welded_run_travel_long`), so the hard limit
holds.

The first and second cases pin the exact boundary: at exactly 100 the pair packs onto the
text line (a form both formatters keep); at 101 the pair travels to its own line.

The third case adds a spaced follower (` tail1 tail2`) after the tag: the pair still
travels, and the follower packs after it on the fresh line — the pair's own trailing
boundary is an ordinary break point and never enters the pair's fit check.

The fourth case glues into a tag whose **expression itself must break** (a wide ternary):
the pair still travels first — the flat measurement fails, so the boundary in front breaks
— and the expression then breaks internally on the fresh line, rather than opening mid-line
(prettier opens the tag mid-line and breaks inside it).

`unformatted_ours_compact.svelte` is the compact authoring (tsv → `input.svelte`, prettier
→ `output_prettier.svelte`).

## Reason

Print width is a hard limit wherever a render-free break exists, and the boundary before
the welded word is render-free.
See [conformance_prettier.md §Print Width Philosophy](../../../../../docs/conformance_prettier.md#print-width-philosophy).
