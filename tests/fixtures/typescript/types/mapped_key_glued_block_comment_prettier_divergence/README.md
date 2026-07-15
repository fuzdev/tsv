# mapped_key_glued_block_comment_prettier_divergence

A run of block comments leading a mapped type's `[K in keyof T]` key, in the bracket-break
path (a line comment on the `[` line is what forces it).

The **run itself matches prettier**: a pair the author glued stays glued and the key breaks
below (`A`), and blocks the author put on their own lines keep them (`B`). That is prettier's
`printLeadingComment` — the separator after each comment is read from the source around *that*
comment, never from where the key starts — which tsv applies through its one shared
leading-comment emitter.

## The divergence

Only the `[`-line comment, unchanged by this fixture's subject: tsv keeps `// force` on the
`[` line, prettier relocates it to its own line as the key's leading comment. That is the
open-delimiter trailing-comment divergence, sanctioned and cataloged for the mapped-type `[`
among the rest of its family.

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
