# chain_trailing_member_gap_line_comment_prettier_divergence

A chain whose base is a call and whose trailing member is not itself called
(`fn().bar`), with a **same-line** line comment in the call→member gap, inside a Svelte
**template island** — an attribute value, a spread, a directive value, an expression
tag, a block head, a prefixed tag, `{@const}`, and a `bind:` value (the one cell that goes
through the block-structure recipe rather than the shared braced-value one).

The lone same-line `//` normally takes the sanctioned collapse (`fn() // c⏎.bar` →
`fn().bar; // c` — see
[trailing_member_after_call_comment](../../../typescript/expressions/calls/chained/trailing_member_after_call_comment_prettier_divergence/)):
the comment defers through `line_suffix` to the end of the output line, and with
nothing else reaching that line end it is lossless. That licence stops exactly where
its argument stops, and a template island is where it stops hardest — the line the
deferral rides to is not the expression's, it is the **host's**. Past the closing `}`
the document is markup, so the deferred `//` lands in the template and comes out as
**rendered page text** (`<a href={fn().bar}>t</a> // c1`) — content the reader sees,
from a comment the author wrote inside an expression. Not a weld and not a
relocation: a different page.

So the chain breaks at the member instead, each comment where the author wrote it —
the same shape every longer chain, a called member, and the own-line spelling of this
gap already take inside an island.

- **tsv**: breaks the chain, indenting the continuation member one level, and keeps
  the `{@const}` chain on the `=` line.
- **prettier**: keeps each comment in place too, but prints the member at the head's
  own indent (`href={fn() // c1⏎.bar}`) and breaks after `=` in `{@const}` — a
  layout-only difference, the same one the `<script>` sibling
  [trailing_member_gap_comment_statement_trailer](../../../typescript/expressions/calls/chained/trailing_member_gap_comment_statement_trailer_prettier_divergence/)
  records.

The `<script>` cell (`// c9`) is the control: the same gap in a `<script>` keeps the
collapse, because there the line the deferral rides to is still this document's own.
The rule is keyed on the host, not on the chain — and it is keyed **default-safe**:
`EmbedContext::printer_owns_line` is `false` unless a printer that genuinely ends its own
lines says otherwise, so a template island is conservative by construction and it is the
`<script>` body that has to claim the collapse.

`unformatted_ours_glued.svelte` is the space-less authoring (`fn()// c1`);
`unformatted_ours_flush.svelte` is prettier's own landing, which tsv re-indents to
`input.svelte` — so the two formatters round-trip through each other's form without
either losing a comment. Both are render-equivalent to `input.svelte`, which is the
property the collapse broke.

Reason: the deferral's licence is that it is lossless, and it is not lossless where the
host owns the rest of the line. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
