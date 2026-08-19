# body_prettier_ignore_paren_comment_prettier_divergence

An arrow whose **body is frozen** by an own-line directive in the `=>`→body gap
([body_prettier_ignore_head](../body_prettier_ignore_head/)) and whose **paren shell** holds
a comment. The freeze ends at the body's own node span, so the pair is outside the slice and
the gap between them is an ordinary paren-shell gap — answered here exactly as the *unfrozen*
arrow body answers it: tsv retains the author's parens and keeps the comment inside them, a
block inline and a `//` with the pair opened around it.

## Why tsv differs

The parting is the unfrozen twin's, unchanged by the freeze — prettier strips grouping parens
and relocates the comment out, floating a `//` past the body's `;` and moving a block outside
a *required* object pair, which re-associates it from the object to the whole expression. That
is already sanctioned at
[arrows/body_paren_comment](../../arrows/body_paren_comment_prettier_divergence/); this fixture
says the frozen form gives that gap the same answer, which is the whole claim about a frozen
value's shell.

Prettier is not idempotent on its own output: its second pass moves each block comment past
the `;` as well, pinned by `audit_signature.txt`.

A frozen **sequence** body's own required pair is the one shape with no oracle here — prettier
throws on a comment in that gap (`Comment "c" was not printed`), the bug recorded at
[init paren comment](../../../statements/variable/init_prettier_ignore_paren_comment_prettier_divergence/).
It takes the same emitter as the cases above.

## Expected behavior

- **tsv**: the frozen slice prints verbatim inside the retained pair, with the shell's comment
  where the author wrote it; the input is a fixed point.
- **prettier**: strips the grouping pairs and relocates each comment out (see
  `output_prettier.svelte`), then moves the blocks again on its second pass.

## Reason

◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *A frozen value's surviving shell*) and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(Arrow body stripped parens).
