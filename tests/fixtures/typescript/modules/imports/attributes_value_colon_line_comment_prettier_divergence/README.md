# attributes_value_colon_line_comment_prettier_divergence

A line comment authored on the `:` line of an import attribute, ahead of the value
(`with { type: // c⏎'json' }`). A `//` runs to end-of-line, so the value must move to
a new line either way.

tsv: keeps the comment where the author put it, trailing the `:`, and hangs the value
one level in below it (`type: // c⏎\t'json'`) — the same answer the declarator `=`
gives (`const a = // c⏎\tb`). The **own-line** authoring keeps its own line instead
(`type:⏎\t// c⏎\t'json'`, the plain [attributes_value_comment](../attributes_value_comment/)
fixture, where prettier agrees), so both placements are stable and neither is moved
onto the other's line.
Prettier: floats the comment *past* the value to trail it (`type: 'json' // c`, the
[variant_trail](./variant_trail.svelte) form — dual-stable, both formatters keep it).

`unformatted_ours_trail` writes the attribute list on one line with the comment still
on the `:` line; tsv breaks the list and keeps the comment there.

## Reason

Per Comment Position Philosophy, tsv keeps a comment where the author wrote it rather
than relocating it across the value to a canonical trailing position — or down onto
its own line. A same-gap *block* comment matches prettier in **both** authorings, so
only a `//` diverges: written on the `:` line a block stays inline
(`type: /* c */ 'json'`, the plain [attributes_comma_comment](../attributes_comma_comment/)
fixture), and written on its own line it keeps that line, hanging the value exactly as
the `//` does (`type:⏎\t/* c */⏎\t'json'`, the plain
[attributes_value_block_comment](../attributes_value_block_comment/) fixture). The
object property's `:` answers the same way
([property_value_colon_line_comment](../../../expressions/objects/property_value_colon_line_comment_prettier_divergence/)).

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
