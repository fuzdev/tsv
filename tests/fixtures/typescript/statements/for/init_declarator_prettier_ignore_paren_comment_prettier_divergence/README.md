# init_declarator_prettier_ignore_paren_comment_prettier_divergence

A `for` header's **init declarator** with a frozen value (`=`→value head,
[init_declarator_prettier_ignore_head](../init_declarator_prettier_ignore_head/)) whose
**surviving paren shell** holds a comment. The freeze ends at the value's own node span, so
the pair around it — the declarator position's clarity parens for an assignment value, or a
sequence's own required pair — is outside the slice, and the gap between the slice and that
`)` is an ordinary paren-shell gap. tsv answers it exactly as the *unfrozen* declarator does:
the comment stays inside the pair, a block inline and a `//` with the pair opened around it.

The header's `ForClauseSeparator` tail is what keeps this honest — a comment here can never
defer past the clause's `;`, because the declarator it was written in ends at that separator
and there is nothing outside it to hold the comment.

## Why tsv differs

There is no prettier output to compare against: prettier **throws** on a comment in this gap
(`Comment "c" was not printed`). Its ignore path replaces the value with the verbatim range
and never visits a comment past that range's end, while the pair still prints — so the
fixture carries `prettier_rejects.txt` rather than `output_prettier.svelte`.

The statement-level declarator and the assignment RHS record the same prettier bug at their
own hosts:
[init paren comment](../../variable/init_prettier_ignore_paren_comment_prettier_divergence/),
[rhs paren comment](../../../expressions/assignment/rhs_prettier_ignore_paren_comment_prettier_divergence/).

## Expected behavior

- **tsv**: the frozen slice prints verbatim inside the surviving pair, with the shell's
  comment where the author wrote it; the input is a fixed point.
- **prettier**: throws.

## Reason

◆comment_preservation ◆prettier_bug. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *A frozen value's surviving shell*).
