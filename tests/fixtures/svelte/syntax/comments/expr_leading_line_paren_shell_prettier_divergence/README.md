# expr_leading_line_paren_shell_prettier_divergence

[expr_leading_line](./../expr_leading_line_prettier_divergence/) reached through a **stripped
paren shell**: the same one line comment in the same head→value gap, but written inside a
redundant grouping paren on the value's **left spine** (`{#if ( // c⏎a).b}` rather than
`{#if // c⏎a.b}`). Both formatters strip the paren, so the two authorings print the same
value — and tsv must reach the same fixed point from both.

tsv, from either authoring:

```svelte
{@html // c
	a.b
}
```

The shell is what makes this its own fixture rather than a variant of the twin. A comment in
the head→value gap sits **before** the value's span; one in a left-spine shell sits **inside**
it — between the node's own start and its leftmost child's, a region holding nothing but the
stripped `(`s and their comments
([comments.md §The left-spine shell run](../../../../../../docs/comments.md#the-left-spine-shell-run-hoisted-outside-the-enclosing-group-and-one-definition-of-left-side)).
The expression's printer hoists that run ahead of the node, so it lands in exactly the
position the plain gap's run lands in, and the head owes it the same continuation indent.
A shell whose pair is **retained** hoists nothing and is deliberately not covered here — its
comment stays inside the parens, which supply their own indent.

`unformatted_ours_paren.svelte` is the shell authoring; tsv normalizes it to `input.svelte` in
one pass. Prettier keeps its own flush continuation, so the variant carries the divergence.

The cases cover the three seams that resolve this gap, since a shell run reaches each of them
by a different route:

- the **block heads** and the **prefixed tags** (`{#if}`, `{#each}`, `{#key}`, `{@html}`,
  `{@render}`), which share one head assembler;
- the **unprefixed `{…}`** — the `{expr}` tag and a plain attribute value — where the space
  before a trailing comment (`{ // c`) is the delimiter's, not an opening literal's, so a head
  that reads as un-broken welds the comment to the brace instead;
- the **`{@const}` init**, which reaches the shape through its break-after-operator layout
  rather than through a head indent.

`{@debug}` is absent by construction: its head takes an identifier list, and an identifier has
no left spine for a shell to sit on.

## Reason

The indent, the run's placement and the `}` column are all the twin fixture's, unchanged — see
[conformance_prettier.md §Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent),
whose **Svelte braced heads** entry carries this rule and the shell clause. What this fixture
adds is that the gate reads the value's **printed** start rather than its span start, so one
authoring cannot answer the question differently from the other.

## Related

- [expr_leading_line](./../expr_leading_line_prettier_divergence/) — the same sweep without the shell, over the whole braced family
- [expr_leading_own_line](./../expr_leading_own_line_prettier_divergence/) — the own-line authoring of the plain gap
- [head_paren_multiline_comment](../../../blocks/head_paren_multiline_comment_prettier_divergence/) — the block-comment shell at the block heads, which forces no break
- [condition_breaking_comment](../../../blocks/if/condition_breaking_comment_prettier_divergence/) — the block head's own fixture for the comment-forced head break
