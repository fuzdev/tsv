# media_list_prettier_divergence

Prettier freezes whatever comment spacing a `@media` prelude was authored with. tsv pads a
comment off the tokens around it at every query-level position, so only the **glued**
authoring diverges.

tsv: `screen /* b */ and`, `not /* f */ screen`, `screen /* h */, print`
Prettier: `screen/* b */and`, `not/* f */screen`, `screen/* h */, print`

The *spaced* authoring agrees on both formatters, including a run inside a feature
expression (`(min-width: /* size */  500px)`): a `media-value` is a node prettier emits
verbatim, so the run there is the author's and tsv keeps it too — pinned by
`variant_spaces` (a form both keep stable) rather than by a `prettier_variant_*`.

## Reason

Stable quirk. tsv normalizes comment spacing consistently across all CSS contexts. This
covers comments between boolean operators, before commas, and inside media features. See
[conformance_prettier_css.md §CSS: Comments](../../../../../../docs/conformance_prettier_css.md#css-comments).
