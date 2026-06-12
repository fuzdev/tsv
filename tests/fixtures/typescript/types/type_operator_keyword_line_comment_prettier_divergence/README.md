# type_operator_keyword_line_comment_prettier_divergence

A line comment after a prefix type operator (`keyof` / `typeof`), before the
operand (`type A = keyof // c\n\t\tB`).

**tsv**: keeps the comment trailing the operator and hangs the operand on the
next line, indented one level — the shared keyword→value layout
(`append_keyword_value_line_comments`):

```
type A = keyof // c
	B;
```

**Prettier**: also keeps the comment after the operator and the operator on the
`=` line, but leaves the operand at the operator's own indent (no hang):

```
type A = keyof // c
B;
```

## Reason

Both formatters preserve the comment after the operator (this is not a
relocation divergence — the comment stays put in both) and keep the operator on
the `=` line. They differ only on the operand's indent: tsv routes the operand
through the same `append_keyword_value_line_comments` mechanism every other
keyword→value site uses (a uniform "hang the value indented under the keyword"
style), while prettier leaves it flush. Both forms are content-preserving and
idempotent in their respective formatters.

A same-line block comment (`keyof /* c */ B`) stays inline in both formatters and
is not a divergence (see the regular
[type_operator_keyword_comment](../type_operator_keyword_comment/) fixture);
only a line comment after the operator diverges. A long *comment-free* operator
still breaks after `=` in both formatters (`type A =\n\tkeyof LongType…`) — the
comment is what keeps the operator on the `=` line here.

Previously tsv emitted the comment inline and **swallowed the operand**
(`type A = keyof // c B;` — `B` absorbed into the comment, a non-idempotent
content loss); keeping it on the operator line via `line_suffix` with the
operand on the next line fixes the loss.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
