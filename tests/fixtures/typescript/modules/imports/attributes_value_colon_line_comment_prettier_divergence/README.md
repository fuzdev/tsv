# attributes_value_colon_line_comment_prettier_divergence

A line comment authored on the `:` line of an import attribute, leading the
value (`with { type: // c⏎'json' }`). A `//` runs to end-of-line, so it can't
trail the `:` inline without swallowing the value — the value must move to a new
line either way.

tsv: keeps the comment leading the value, breaking after `:` and indenting the
value one level (`type:⏎\t// c⏎\t'json'`) — the same layout as when the author
writes the comment on its own line (the plain
[attributes_value_comment](../attributes_value_comment/) fixture; both authorings
reach one fixed point).
Prettier: floats the comment *past* the value to trail it (`type: 'json' // c`,
the [variant_trail](./variant_trail.svelte) form — dual-stable, both formatters
keep it).

## Reason

Per Comment Position Philosophy, tsv keeps a comment where the author wrote it —
leading the value — rather than relocating it across the value to a canonical
trailing position. A same-gap *block* comment stays inline and matches prettier
(`type: /* c */ 'json'`, the plain
[attributes_comma_comment](../attributes_comma_comment/) fixture); only a `//`,
which forces the break, diverges.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
