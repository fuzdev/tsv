# end_of_line_block_comment_prettier_divergence

Prettier moves a block comment that ends an array element's line *across the element's
comma*, from leading the next element to trailing the previous one. `['aaaa', /* c */⏎'bbbb']`
becomes `['aaaa' /* c */, 'bbbb']` — the comment was written about `'bbbb'` and now reads as
being about `'aaaa'`. Prettier classifies on newlines alone (`endOfLine`: content before the
comment on its line, none after), so the comma — which is what actually carries the
association — plays no part.

We preserve the comment's position: it stays after the comma, leading `'bbbb'`.

Both positions are dual-stable: `['aaaa', /* c */ 'bbbb']` and `['aaaa' /* c */, 'bbbb']` are
each idempotent under both formatters (`variant_before_comma`). The divergence is in
normalization — prettier normalizes the newline-after form to before the comma, while we
normalize it to after the comma (`unformatted_ours_newline_after`).

An authored **blank** line after the comment separates the two facts the comment carries. The
comment still leads the element, so it stays after the comma and takes its own line; the blank
line is authorship *about the element* and survives between them (`h`). Only the comment's own
line break is ours to reflow.

That is the opposite of a **value** gap, where the same blank yields with the break
([conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position)),
and the contrast is deliberate: here the blank sits between two *list items*, which is ordinary
blank-line authoring tsv preserves; there it sits inside a head→value break the formatter has
already judged unforced, leaving no two lines for it to separate. The printer names the split as
the leading-run modes `Adjacent` (this case) and `AdjacentValueGap`.

This is *not* the sanctioned pure-separator trail. That carve-out covers a same-line **line**
comment (`A // c⏎, B` → `A, // c`), where the comment trails `A` in both forms — only the comma
slides across it, the binding never changes, and a `//` runs to end-of-line so no other
rendering exists. A block comment has neither property: the binding flips, and
`['aaaa', /* c */ 'bbbb']` renders perfectly well. Relocating it is unforced.

The list case also inverts the usual newline-is-intent reading. tsv owns the line breaks inside
an array and reflows them, so the author's newline after the comment does not survive
formatting — but the comment's position relative to the comma does. Deciding the durable fact
from the ephemeral one is backwards.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).
