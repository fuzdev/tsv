# left_spine_paren_prettier_ignore_interior_prettier_divergence

An own-line directive the author wrote inside the **grouping parens the parser erases**
ahead of a construct's leftmost operand. Those parens leave no node behind, so the directive
lands *inside* the construct's own span — where no enclosing value head can see it — and the
construct itself answers for it: the operand it precedes freezes, and the run keeps the line
the author gave it, hoisted ahead of the construct exactly where the reparse reads it back.

```ts
const d = (
	// prettier-ignore
	{x:   2}
).f;
```

`unformatted_ours_paren_shell.svelte` is that parenthesized authoring across seven hosts — a
ternary's test, a member chain's base, a non-null assertion's operand, a tagged template's
tag, a sequence's first operand, a chain base the printer needs a pair around, and a bare
callee — plus the ternary host one level down, a **nested** conditional's test in a `?` / `:`
branch (`const q` / `const u`, and the block spelling `const z`); `input.svelte` is the form
every cell converges to, in ONE pass. Both tools hold the seven root cells; the nested cells
are where prettier parts on `input.svelte` too (below).

At the nested host the erased shell is the enclosing branch gap's, so the run hoists into that
gap on its own line and the nested test freezes from its own scan. The next pass reads the
hoisted directive as the branch head's and freezes the whole branch — the same coarser claim
the root cells reach through the value head. The gap's layout gate reads the shell as well,
which is what forces the parent open for the block spelling.

The freeze scope is that one operand, the construct the directive precedes: the whole
construct is not a candidate, since its slice would have to contain the `)` the strip
removes. So the operands AROUND the frozen one normalize — the sequence cell pins that
(`{x:   4}` frozen, `{ y: 5 }` normalized). A pair the printer REQUIRES is its own, not the
author's, so it lands outside the slice and survives the freeze (the `const k` cell, whose
ternary base could not be printed bare).

Two siblings hold the rest of the shape: the binary chain's first operand, the one host
prettier converges with, is the non-divergence
[left_spine_paren_prettier_ignore_interior](../left_spine_paren_prettier_ignore_interior/),
and a pair the printer **retains** — which keeps the frozen operand inside it and diverges in
tsv's own output — is
[left_spine_retained_paren_prettier_ignore_interior](../left_spine_retained_paren_prettier_ignore_interior_prettier_divergence/).

## Why tsv differs

Prettier relocates the directive out of the parens to trail the `=`, and never comes back:
its own second pass reformats the operand the directive froze, so the freeze is lost on every
root host (the sequence cell goes further and demotes the directive to a statement-trailing
comment). `audit_signature_paren_shell.txt` pins that chain, which never reaches
`input.svelte`. At the nested host prettier pulls the directive onto the operator's line in
both spellings and holds it there — a placement it honors and tsv's floor calls inert
(`output_prettier.svelte`, its second pass pinned by `audit_signature.txt`). A head-trailing placement is **inert** under tsv's classification, so the
author's own-line placement is the only one that holds the freeze across a second pass — and
one authoring of a claim should not reach a different fixed point than the other.

## Reason

◆comment_preservation ◆prettier_bug — sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On delimiter-owned value heads, and on sequence operands*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
