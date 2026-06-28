# type_param_keyword_own_line_comment_prettier_divergence

An **own-line** leading comment between a type parameter's `extends`/`=` keyword
and its constraint/default value (`R extends\n// c\nA`, `U =\n// c\nV`).

**tsv**: keeps the comment on its own line, in the indented value block:

```
U =
	// c
	V
```

**Prettier**: pulls the first leading comment up onto the keyword line:

```
U = // c
	V
```

Prettier is **non-idempotent** getting there — its first pass lands the value at
the param's own indent (`U = // c\n\tV`), and a second pass adds the extra indent
(`audit_signature.txt` pins the pass-2 fixed point). tsv stays idempotent.

Per Comment Position Philosophy: the user wrote the comment on its own line, so
tsv preserves that placement rather than collapsing it onto the keyword line.
A comment that is *already* on the keyword line (`U = // c\n…`): tsv emits it
inline via `line_suffix` (zero width, so a long trailing comment never forces a
preceding constraint union to break). Prettier 3.9 instead breaks the `extends`
constraint under a long trailing comment, so that case is a separate divergence;
see [type_param_keyword_line_comment](../type_param_keyword_line_comment_prettier_divergence/).
Only an own-line first comment diverges here.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
