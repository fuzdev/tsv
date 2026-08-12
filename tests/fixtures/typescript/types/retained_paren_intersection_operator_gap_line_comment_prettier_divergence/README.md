# retained_paren_intersection_operator_gap_line_comment_prettier_divergence

A **line** comment in a member→member (`&`) gap of a **retained** parenthesized
intersection whose last member is an **object literal** — the shell that prints its
own `(`…`)` and aligned object layout
([retained_paren_intersection_member_comment](../retained_paren_intersection_member_comment_prettier_divergence/)
covers that shell's leading and trailing gaps; this one covers the operator gaps
between its members).

**Prettier** relocates a comment sharing the `&`'s line out of the parens, to trail
the whole member:

```
type B1 =
	| (a & { x: X }) // c
	| c;
```

**tsv** keeps it on the line the author gave it, so the trailing object drops to the
next line:

```
type B1 =
	| (a & // c
		{ x: X })
	| c;
```

Per Comment Position Philosophy, the comment sits between the operator and its
operand and stays there — the same rule the **bare** (paren-free) intersection takes
in [intersection_member_operator_gap_comment](../intersection_member_operator_gap_comment_prettier_divergence/),
and the same keep-inside-the-shell rule its sibling gaps take. A `//` runs to
end-of-line, so the object cannot share that line and the shell expands with it.

`B2` — an **own-line** comment between the `&` and the object — **matches** prettier
(both keep it on its own line above the object); it is here because it exercises the
same gap, and its presence is what shows the divergence is about the comment's
authored line, not about the gap.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
