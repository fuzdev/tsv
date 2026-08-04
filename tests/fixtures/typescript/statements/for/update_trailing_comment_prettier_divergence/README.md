# update_trailing_comment_prettier_divergence

A comment between the update clause and the header's `)` stays inside the header:
on its own line before `)` when the author gave it one, trailing the update when it
shares the update's line. Prettier moves an own-line one out of the header entirely,
stranding it between `)` and the body `{`.

tsv: keeps the comment inside the parens, on the line the author gave it
Prettier: relocates an own-line comment past `)`, before the body

## Reason

The update→`)` gap is the update clause's own region — the header's closing `)` is
all that terminates it, exactly as with an
[empty update slot](../empty_slot_comment_prettier_divergence/), where prettier
relocates the same way. Not preserving drops the comment: the gap has no other
emitter (only the same-line trail did), which is what tsv used to do.

A comment sharing the update's line trails it in both formatters — the third case is
a control, not a divergence.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
