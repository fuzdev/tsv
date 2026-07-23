# type_operator_keyword_line_comment_prettier_divergence

A line comment in a prefix type operator's (`keyof` / `typeof`) keyword→operand
gap, the operand on a later line. Two authorings, two divergence characters:

- **Trailing** the operator (`keyof // c\n\t\tB`) — an **indent-only** divergence:
  the comment stays put in both formatters, they differ only on the operand's
  indent.
- On its **own line** (`keyof\n\t\t// c\n\t\tB`) — a **relocation + indent**
  divergence: prettier pulls the comment *up* onto the operator line, tsv keeps it
  where the author wrote it.

## tsv

Keeps the comment where the author wrote it and hangs the operand on the next
line, indented one level — the shared keyword→value layout
(`append_keyword_value_line_comments`):

```
type A = keyof // c
	B;

type C = keyof
	// c
	B;
```

## Prettier

Leaves the operand **flush** at the operator's own indent (no hang), and for the
own-line authoring pulls the comment *up* onto the operator line:

```
type A = keyof // c
B;

type C = keyof // c
B;
```

## Reason

**Trailing form** (`type A` / `type B`): both formatters preserve the comment
after the operator (not a relocation — the comment stays put in both) and keep
the operator on the `=` line. They differ only on the operand's indent: tsv routes
the operand through the same `append_keyword_value_line_comments` mechanism every
other keyword→value site uses (a uniform "hang the value indented under the
keyword" style), while prettier leaves it flush.

**Own-line form** (`type C` / `type D`): tsv keeps the comment on its own line
where the author wrote it (per the
[Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)),
hanging the operand below; prettier relocates the comment up onto the operator
line and leaves the operand flush — the same up-pull it applies to an own-line
type-parameter keyword comment and to `infer` (see
[infer/keyword_line_comment](../infer/keyword_line_comment_prettier_divergence/)).

Both forms are content-preserving and idempotent in their respective formatters. A
same-line block comment (`keyof /* c */ B`) stays inline in both formatters and is
not a divergence (see the regular
[type_operator_keyword_comment](../type_operator_keyword_comment/) fixture); only a
line comment after the operator diverges. A long *comment-free* operator still
breaks after `=` in both formatters (`type A =\n\tkeyof LongType…`) — the comment
is what keeps the operator on the `=` line here.

Emitting the comment inline instead would **swallow the operand**
(`type A = keyof // c B;` — `B` absorbed into the comment, a non-idempotent
content loss); keeping it on the operator line via `line_suffix` with the operand
on the next line avoids the loss.

A redundant paren wrapping the operand with the line comment inside
(`keyof (// c\n B)`, and the double-nested `((…))`) strips to this same fixed
point — the `unformatted_ours_single_paren` / `unformatted_ours_double_parens`
variants verify the paren form is idempotent too. A paren that is *required*
(e.g. `keyof (A | B)`) is re-added around the hung operand rather than dropped.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation
("Prefix type-operator operand hang" — layout for the trailing form, relocation
for the own-line form).
