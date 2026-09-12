# init_assignment_prettier_ignore_paren_comment_prettier_divergence

A `for` header's **assignment** RHS whose grouping shell holds a `//`: the shell is
RETAINED (a bare line comment would swallow the `)`), and the retained shell is already the
`[~In]` pair the header's init owes — so exactly one pair prints. The frozen and unfrozen
forms answer it identically, and so does the declarator twin, which reaches the same shell
through the header's own builder.

The **unfrozen** cells are not controls: they pin a fix of their own. Both spellings used to
print two pairs (`((…))`), the assignment builder wrapping a shell that had already
supplied the pair, and both now print one. The declarator cell is the control — it always
printed one, because its shell builder owns the wrap on that path.

## Why tsv differs

◆comment_preservation. The parting is the one
[init declarator redundant paren comment](../init_declarator_prettier_ignore_redundant_paren_comment_prettier_divergence/)
already records, at the assignment host: on the `//` spelling tsv retains the shell and
keeps the comment inside it, where prettier strips the shell anyway and carries the comment
out past the clause's `;` onto a line it does not own. Prettier is not idempotent on its own
output — its second pass collapses the unfrozen declarator and assignment onto one line
(`audit_signature.txt`).

## Expected behavior

- **tsv**: one pair on every cell, the comment inside it, the input a fixed point.
- **prettier**: strips the shell, re-adds its own pair, floats the comment past the `;`
  (`output_prettier.svelte`), then reflows on a second pass.

The `/* … */` spelling is deliberately absent: a block comment in that gap defers past the
clause `;` and is not a fixed point under either tool, an unrelated pre-existing gap tracked
by a `// TODO:` on `ShellTail::StatementTerminator` (the for-header assignment RHS takes the
statement-terminator tail, where the declarator takes the clause-separator one).

## Reason

◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *A frozen value's REDUNDANT shell*) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
