# trailing_member_comment_prettier_divergence

Prettier relocates line comments before trailing member access in call chains. tsv preserves comments where the user placed them.

tsv: `.filter((x) => x)\n// comment\n.length` (preserved)
Prettier: `// comment\nitems.filter((x) => x).length` (relocated before chain)

Prettier's relocation shape depends on the trailing member: a plain `.length`
gets the comment on its own line under `=`; an **optional** `?.length` instead
trails the comment on the `=` line itself (`const b = // comment`) and de-indents
the value (changed in prettier-plugin-svelte 4.x — 3.5.2 used the own-line form
for both). tsv preserves the user's placement in either case.

That optional-chain shape is also **non-idempotent** under prettier: the first pass
puts the value at one indent, a second pass re-indents it to two — so prettier never
reaches a fixed point in one pass. `output_prettier.svelte` records pass 1 and
`audit_signature.txt` pins the full chain to its `PASS=2` fixed point (rule F4). tsv
is idempotent (input formats to itself).

## Reason

Comment relocation. tsv treats user comment placement as intentional. Consistent with tsv's handling across if/else, try/catch, switch, for, while, do-while, labeled statements, and other chain comment contexts.

See [conformance_prettier.md §Comment relocation](../../../../../../../docs/conformance_prettier.md#comment-relocation).
