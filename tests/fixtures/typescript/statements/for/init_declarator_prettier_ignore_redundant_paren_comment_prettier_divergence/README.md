# init_declarator_prettier_ignore_redundant_paren_comment_prettier_divergence

The **redundant** half of the frozen init declarator's shell
([the surviving one](../init_declarator_prettier_ignore_paren_comment_prettier_divergence/)):
a paren the frozen value does not need, retained here because the comment inside it is a
`//`. Inline, that comment would swallow the `)`; and unlike a statement-level declarator,
a header declarator has no terminator to defer it past — the clause's `;` ends the
declarator the comment was written in, and past it there is nothing to hold the comment.

Prettier strips the shell anyway and floats the `//` past that separator onto the header
line it does not own (`bbb  +  ccc; // c`).

A block comment in the same gap needs no shell and **matches** prettier — that case is the
ordinary sibling [init_declarator_prettier_ignore_head](../init_declarator_prettier_ignore_head/).

## Why tsv differs

The same parting the unfrozen twin already records at this host
([init_paren_line_comment](../init_paren_line_comment_prettier_divergence/)): tsv keeps a
comment inside the pair the author wrote rather than carrying it across a separator, where
it would re-bind to whatever follows.

## Expected behavior

- **tsv**: the shell is retained, the frozen slice prints verbatim inside it, and the `//`
  keeps its own line inside the pair; the input is a fixed point.
- **prettier**: strips the shell and floats the comment past the clause separator (see
  `output_prettier.svelte`).

## Reason

◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *A frozen value's REDUNDANT shell*).
