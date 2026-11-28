# union_prettier_ignore_nested_paren_member_prettier_divergence

A frozen union member that is itself a parenthesized union — prettier's own
`tests/format/typescript/prettier-ignore/prettier-ignore-nested-unions.ts`. Adapted: the
upstream `// baz` before `type b`'s last member is omitted — it exposes a separate indent
issue, not this divergence.

- `type a` — the directive before the member: **tsv** keeps the author's whole member
  verbatim, the paren layout as written. **Prettier** re-synthesizes the parens tight around
  its frozen slice, which starts at the inner leading `|` (`(| aaaaaaaaaaaaa&1`), and moves
  the `)` onto the last frozen line.
- `type b` — the directive inside the parens: **tsv** keeps it where the author wrote it and
  applies the member rule to the inner list, so only the inner member after it freezes and
  the later inner members reformat. **Prettier** hoists the directive above the outer member
  and freezes the inner union from its leading `|`.

`output_prettier.svelte` records prettier's form; it is a fixed point of both formatters (the
bytes it moved are frozen bytes to tsv).

## Reason

Design choice and comment preservation: the directive binds to the member it precedes, and an
in-paren directive is a comment of the inner list, kept in the position the author gave it.
See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
