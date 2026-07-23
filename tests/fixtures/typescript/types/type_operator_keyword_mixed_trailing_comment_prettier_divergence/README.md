# type_operator_keyword_mixed_trailing_comment_prettier_divergence

The mixed / trailing extension of
[type_operator_keyword_line_comment](../type_operator_keyword_line_comment_prettier_divergence/):
a redundant paren shell around a prefix type-operator operand (`keyof`/`typeof`/
`readonly`) whose leading gap holds a **block comment before the line comment**
(mixed), or whose trailing gap holds a **block comment after the operand**
(trailing).

**tsv**: strips the shell and hangs the whole comment run at the same fixed point
the bare (paren-free) authoring settles on — the block trails the operator inline,
the line comment forces the operand onto the next line indented one level, and a
trailing block trails the operand:

```
type A = keyof /* b */ // c
	B;

type C = keyof // c
	D /* t */;
```

**Prettier**: keeps the comments in place but leaves the operand flush (no
continuation indent) — the same indent-only divergence as the pure-line sibling:

```
type A = keyof /* b */ // c
B;

type C = keyof // c
D /* t */;
```

The operand hangs one level under the operator (the uniform keyword→value layout,
`append_keyword_value_line_comments`), where prettier leaves it flush. The
`unformatted_ours_single_paren` / `unformatted_ours_double_parens` variants verify
the paren shells are idempotent under tsv.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation
(Prefix type-operator operand hang).
