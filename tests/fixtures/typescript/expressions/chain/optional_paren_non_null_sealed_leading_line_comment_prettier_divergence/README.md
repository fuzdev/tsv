# Sealed optional-chain non-null, line comment in the leading gap

The leading-gap sibling of
[optional_paren_non_null_sealed_line_comment](../optional_paren_non_null_sealed_line_comment_prettier_divergence/):
a `//` between the sealed shell's `(` and the chain (`new ( // c⏎a?.b)!()`), at
the positions the parens are required — a `new` callee and a template tag.

- **tsv**: keeps the comment inside the parens and takes the expanded shell —
  the comment on the `(` line (glued) or its own line (own-line authoring), the
  chain one indent in, the `)!` back out — the same layout the trailing-gap
  sibling already renders, and the same one every required pair in the family
  takes (the
  [assignment target](../../assignment/cast_target_leading_line_comment_prettier_divergence/),
  the [instantiation head](../../../typescript_specific/generics/instantiation_head_paren_leading_line_comment_prettier_divergence/)).

```
new ( // c1
	a?.b
)!();
```

- **prettier**: also keeps the comment inside the pair, but glues it to the `(`
  with no space and leaves the continuation flush (`new (// c1⏎a?.b)!();`),
  pulling an own-line comment up to the `(` line on the way — so the divergence
  is the rendering alone; no comment leaves the pair.

An inline block run leads the chain flat and matches prettier byte-for-byte —
pinned here as the controls (`new (/* c4 */ /* c5 */ a?.b)!();`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
