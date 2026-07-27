# empty_slot_comment_prettier_divergence

A comment written in an **empty clause slot** of a C-style `for` header — between
`(` and the first `;`, between the two `;`, or after the second `;` — stays in that
slot. Prettier moves it out: into the next clause across the `;` (empty init and
empty test slots) or out of the header entirely, stranding it between `)` and the
body `{` (empty update slot).

tsv: keeps the comment in the slot the author wrote it in
Prettier: relocates it to the following clause, or outside the header

## Reason

The slot a comment sits in is what it is about — a comment in the empty test slot
documents the missing condition, not the update expression prettier binds it to.
Relocating across a `;` changes that association, and the empty-update relocation
strands the comment outside the construct entirely.

This is the partially-empty header's form of the rule the fully-empty header already
follows ([empty_clauses_comment](../empty_clauses_comment_prettier_divergence/),
[empty_clauses_block_comment](../empty_clauses_block_comment_prettier_divergence/)),
where prettier likewise relocates every comment outside the parens.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
