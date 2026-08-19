# `await`→operand, format-ignore head with a surviving paren shell

An `await` whose **operand is frozen** by an own-line directive in the keyword→operand gap
([await_new_operand_prettier_ignore_head](../await_new_operand_prettier_ignore_head_prettier_divergence/))
and whose **paren shell** holds a comment. The freeze ends at the operand's own node span,
so the pair is outside the slice and the gap between them is an ordinary paren-shell gap —
answered here exactly as the *unfrozen* `await` operand answers it: tsv retains the author's
parens and keeps the comment inside them, a block inline and a `//` with the pair opened
around it.

## Why tsv differs

The parting is the unfrozen twin's, unchanged by the freeze — prettier strips the grouping
parens and relocates the comment out, floating a `//` past the operand's `;` and moving a
block outside the pair. The shell is not optional for tsv: `await`'s own span covers the `)`,
so a stripped form would hand the comment to the enclosing terminator gap on reparse and the
authoring would have no fixed point at all.

This is the already-sanctioned frozen-value's-surviving-shell class one host over from
[arrow body_prettier_ignore_paren_comment](../arrow/body_prettier_ignore_paren_comment_prettier_divergence/),
which says the same thing about the `=>`→body gap. The last case adds the shape the arrow
has no counterpart for: an operand whose parens are **required** (`await (fff ?? ggg)`),
where prettier strips the author's pair, re-emits the required one, and lands the comment
outside it.

## Expected behavior

- **tsv**: the frozen slice prints verbatim inside the retained pair, with the shell's
  comment where the author wrote it; the input is a fixed point.
- **prettier**: strips the grouping pairs and relocates each comment out (see
  `output_prettier.svelte`), and is not idempotent on its own output — pinned by
  `audit_signature.txt`.

## Reason

◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *A frozen value's surviving shell*) and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(await grouped operand); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
