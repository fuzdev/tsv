# attributes_key_colon_line_comment_prettier_divergence

A line comment in an import attribute's key→`:` gap (`with { type // c⏎: 'json' }`).
A `//` runs to end-of-line, so `: value` must move to a new line either way.

tsv: keeps the comment trailing the key and drops `: 'json'` to a continuation
line indented one level (`type // c⏎\t: 'json'`) — the uniform forced-continuation
indent shared with the object-property and type-member key→`:` line-comment paths
(`{ a // c⏎: b }`, `type T = { a // c⏎: B }`).
Prettier: relocates the comment past the value to trail it (`type: 'json' // c`,
the [output_prettier](./output_prettier.svelte) form).

## Reason

Per Comment Position Philosophy, tsv keeps a comment where the author wrote it —
in the key→`:` gap — rather than floating it across the value to a canonical
trailing position. A same-gap *block* comment stays inline and matches prettier
(`type /* c */: 'json'`, the plain
[attributes_comma_comment](../attributes_comma_comment/) fixture); only a `//`,
which forces the break, diverges. The sibling
[attributes_value_colon_line_comment](../attributes_value_colon_line_comment_prettier_divergence/)
covers the `:`→value gap.

The **own-line** authoring (`type⏎// c⏎: 'json'`) pulls up to trail the key and
reaches input under tsv — one pass (`unformatted_ours_own_line.svelte`).
Prettier instead crosses the `:` and hangs the comment leading the value
(`type:⏎\t// c⏎\t'json'`) — stable in one pass, and dual-stable
(`variant_own_line.svelte`), the same landing its own-line **block** sibling
[attributes_key_colon_own_line_block_comment](../attributes_key_colon_own_line_block_comment_prettier_divergence/)
pins — distinct from the trailing float it applies to input's authoring
(`output_prettier.svelte`).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
