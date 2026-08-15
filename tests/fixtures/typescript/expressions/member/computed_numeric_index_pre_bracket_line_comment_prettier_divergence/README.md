# computed_numeric_index_pre_bracket_line_comment_prettier_divergence

A line comment in the gap between a call and the `[` of a computed access whose index
is a **numeric literal** (`foo.bar().baz()⏎// c⏎[0]`) — the numeric-index counterpart
of [computed_pre_bracket_line_comment](../computed_pre_bracket_line_comment_prettier_divergence/).

The index kind is the whole point. Following prettier's member-chain grouping, a
computed access with a numeric-literal index is **glued into the preceding call's
group** instead of starting a new one — so, unlike every other member, no chain group
would begin at its `[`, and nothing in the chain builder would own the gap before it:
the one place a chain's line comment could be deferred to end of line. tsv's grouping
therefore lets a line comment in that gap start a group (prettier's own rule — a node
with a trailing comment closes its group), and the trailing group's forced pre-bracket
break then renders inside the chain's indent.

**tsv** breaks the chain and keeps each comment where the author wrote it, exactly as
it does for a non-numeric index: an own-line comment keeps its own line before the `[`,
a same-line comment trails the call (case `c`), and two comments in one gap stay
distinct and in order (case `b`). The rule does not depend on chain length: on a
**short** chain — a lone call base (`fn()`), an identifier base (`arr`), a same-line
comment, statement position (cases `d`–`g`) — the comment and the bracket still indent
one level below the base, exactly as an identifier index (`[i]`) does. The compact
authoring (`unformatted_ours_compact`) converges to `input.svelte` in one pass.

**Prettier** has no fixed point here at all — see `prettier_nonconvergent.txt`. It
relocates the comment *inside* the brackets (`foo.bar().baz()[// c1`, with `0];` on the
next line), which re-formats again on the next pass.

## Reason

Per Comment Position Philosophy, tsv keeps the comment where the author wrote it. A
`//` must end its line, so the bracket drops to the next line — deferring the comment
to end of line instead would relocate it past both the brackets and the `;`, and two
such comments would **merge onto one line, where the first `//` swallows the second**
(silent content loss). Prettier's own non-convergence here means there is no oracle to
track even if tsv wanted to.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
