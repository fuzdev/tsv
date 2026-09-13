# left_spine_paren_prettier_ignore_required_pair_prettier_divergence

A frozen leftmost operand takes the paren pair its **position** requires, outside the slice —
the `needs_parens` verdict its unfrozen twin would have taken, plus the one pair the SLICE
needs beyond it. The slice replaces the very builder that asks that question, so a freeze
that skips it drops a pair the position requires and the output re-binds.

```ts
const a = (
	// prettier-ignore
	new A
).b;
```

`unformatted_ours_paren_shell.svelte` is that parenthesized authoring across every host and
`input.svelte` is the form they converge to, in ONE pass. Both tools hold `input.svelte`, so
the divergence here is entirely what prettier does with the parenthesized spelling.

The **argument-less `new`** cells (`.b`, `()`, `!`, `[0]`, `` `t` ``, and the arrow-body one)
are the pair the SLICE needs that the node does not: an unfrozen `new A` prints as `new A()`,
so `needs_parens` leaves it bare at every suffix position, while a verbatim slice carries only
the author's bytes and hands `.b` to the CALLEE — `new A.b` is `new (A.b)`.

A `new` the author wrote with **type arguments** ends its slice at `>`, and the question is
then which tails JOIN that `>` rather than which operators bind tighter. `(new DD<T>) + 1` and
`(new DD<T>)[0]` do — bare, `+` opens the `>`'s right-hand side and the whole thing reads as
`((new DD) < T) > +1` — while `new DD<T> ** 2` and `new DD<T> ? ii : jj` do not: neither `**`
nor `?` can open an expression there, so the parse backtracks to the type arguments and the
bare form is the same tree. Those two cells pin the pair's ABSENCE. An **instantiation** member
object (`(ll<T>).nn`, `(ll<T>)[0]`) takes its pair from the chain, which requires one there.

The remaining cells are the position's own precedence pair over an operand the grammar binds
the other way: a binary operand at a template tag, a ternary's test and a `**` left operand
(both right-associative), and the ambient `[~In]` pair a `for` header's init owes.

The last two cells are the no-double-pair guard: an expression statement's leftmost target and
an arrow body's each ask for a pair of their own around the same slice, and exactly one is
emitted — a parenthesized operand can no longer open the statement's line with `{`, nor read as
an arrow's block body.

The converged siblings are the ordinary-match
[left_spine_paren_prettier_ignore_required_pair](../left_spine_paren_prettier_ignore_required_pair/),
whose hosts prettier reaches in one pass too, and the base shapes in
[left_spine_paren_prettier_ignore_interior_prettier_divergence](../left_spine_paren_prettier_ignore_interior_prettier_divergence/).

## Why tsv differs

Prettier's ignore path emits the frozen slice without re-asking `needsParens` for the argument-less
`new`, so every one of those cells comes back **re-bound**: `new A.b`, `new F!`, `new H[0]`,
`` new J`t` ``, `new Z.aa`, `new DD<T> + 1` and `new DD<T>[0]` are all a different tree from the
input, and none of them is a form tsv can follow — a formatter's output has to mean what its
input meant. Its own next pass reads the re-bound form back as a `new` with a member callee and
appends the argument list, compounding the change to `new A.b()`, `new F!()`,
`` new J`t`() ``, and for the type-argument cells reading the `<T>` as two comparisons
(`new DD() < T > [0]`). On the remaining hosts prettier keeps the
pair and relocates the directive to trail the `=` instead, a placement **inert** under tsv's own
classification, so following it would lose the freeze on tsv's second pass; the `for`-init and
`**` cells keep the directive on its own line and break after the operator instead.
`audit_signature_paren_shell.txt` pins that whole chain, which reaches `input.svelte` at no pass.

## Reason

◆comment_preservation ◆prettier_bug — sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On delimiter-owned value heads, and on sequence operands*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy).
