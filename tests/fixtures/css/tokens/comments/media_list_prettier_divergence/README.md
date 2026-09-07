# media_list_prettier_divergence

Prettier freezes whatever comment spacing a `@media` prelude was authored with. tsv pads a
comment off the tokens around it at **query level** — between a media type, a boolean
keyword and a feature expression — so only the glued authoring of those positions diverges.

tsv: `screen /* b */ and`, `not /* f */ screen`, `screen /* h */, print`
Prettier: `screen/* b */and`, `not/* f */screen`, `screen/* h */, print`

Inside a feature expression the two **agree**: that text is a `media-feature` or
`media-value` node prettier emits with no insertion of its own, and tsv takes the same
split, so a comment glued to a paren, a name or a value stays glued and a value's run
stays the author's (`(min-width: /* size */  500px)`, pinned by `variant_spaces`, a form
both keep stable). The glued interior authorings are pinned by
[media_feature_comment_glued](../../../at_rules/media_feature_comment_glued/).

## Reason

Stable quirk. tsv normalizes comment spacing consistently across all CSS contexts, and at
query level that means one space on each side. See
[conformance_prettier_css.md §CSS: Comments](../../../../../../docs/conformance_prettier_css.md#css-comments).
