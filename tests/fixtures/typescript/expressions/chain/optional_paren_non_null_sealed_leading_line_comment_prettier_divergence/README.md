# Sealed optional-chain non-null, line comment in the leading gap

The leading-gap sibling of
[optional_paren_non_null_sealed_line_comment](../optional_paren_non_null_sealed_line_comment_prettier_divergence/):
a `//` between the sealed shell's `(` and the chain (`new ( // c⏎a?.b)!()`), at
every position the parens are required — a `new` callee, a template tag, and the
base of a member chain.

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

The **member-access** position reaches the same gap through the chain printer —
the sealed parenthesized base — rather than through a wrapper node's own shell,
and takes the same layout, with the assignment hugging it (`const m = ( // c7`)
exactly as the [trailing-gap sibling](../optional_paren_non_null_sealed_line_comment_prettier_divergence/)
already pins. The base's pair is required with or without the `!` (`(a?.b).ddd`
seals the chain on its own), so both spellings answer the gap the same way; here
prettier breaks after the `=` before its compact shell.

The **template tag** is required with or without the `!` too (`` (a?.b)`tpl` ``
seals the chain on its own), and takes the same shell — c10.

An inline block run leads the chain flat and matches prettier byte-for-byte —
pinned here as the controls (`new (/* c4 */ /* c5 */ a?.b)!();`, and at the base
`const o = (/* c9 */ a?.b)!.ccc;`).

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
