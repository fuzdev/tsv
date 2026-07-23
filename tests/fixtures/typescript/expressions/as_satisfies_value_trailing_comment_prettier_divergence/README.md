# as_satisfies_value_trailing_comment_prettier_divergence

The trailing extension of
[as_satisfies_value_line_comment](../as_satisfies_value_line_comment_prettier_divergence/):
a redundant paren shell around an `as`/`satisfies` cast type whose leading gap
holds a line comment **and** whose trailing gap holds a block comment after the
type (`(// c\n A /* t */)`, and the double-nested form).

**tsv**: strips the shell and hangs the run at the same fixed point the bare
authoring settles on. The line comment forces the type onto the next line; the
trailing block, because a cast is a value position, **defers past the statement
`;`** (via `line_suffix`) — matching the declarator's own value→`;` trailing
handling, so the form is idempotent:

```
const a = x as // c
	A; /* t */
```

**Prettier**: floats every comment out. Its output on the paren shell is
non-idempotent — a first pass lands the block before the `;`
(`x as A /* t */; // c`, the `prettier_intermediate_to_variant_*` files) and a
second pass moves it past the `;` (`x as A; /* t */ // c`, the dual-stable
`variant_*` files):

```
const a = x as A; /* t */ // c
```

Per Comment Position Philosophy, tsv keeps the comments associated with the cast.
The `unformatted_ours_*` variants verify the paren shells are idempotent under
tsv; the `variant_*` / `prettier_intermediate_to_variant_*` siblings pin
prettier's two-pass float.

See [conformance_prettier.md](../../../../../docs/conformance_prettier.md) §Comment relocation.
