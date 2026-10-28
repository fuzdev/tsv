# Sealed chain base, comment in a stripped shell around its member

A comment in grouping parens the parser strips from around a sealed chain's
member (`(/* c */ (x?.y).p).t()`), in a chain that holds a call. The
`unformatted_ours_stripped_shell` variant holds those authorings; `input.svelte`
is where tsv prints them, in one pass.

- **tsv**: the comment prints once, in front of the member the shell wrapped —
  `(x?.y) /* c1 */.p⏎\t.t();` — exactly where it lands without the seal
  (`(/* c */ (x.y).p).t()` prints `x.y /* c */.p⏎\t.t();`, prettier's own fixed
  point, [block_comment_chain](../../calls/chained/block_comment_chain_prettier_divergence/)).
  It sits *after* the `)` that seals the chain, so it stays outside the pair, as
  an authored `(x?.y) /* c */.p` does. The same answer holds for a `!` seal (c5),
  a computed lookup (c6), a deeper member (c7), an IIFE callee (c8), the other
  base that prints its own pair, a chain a `!` ends (c16), both line-comment
  placements (c11, c12), an optional continuation (c17), the other sealed bases
  (c18, c19), a multi-line block (c20), a call with type arguments (c21) and a
  template expression (c15). It takes that place only inside a member chain a
  call roots and only in authored order
  ([stripped_shell_comment_source_order](../../calls/chained/stripped_shell_comment_source_order_prettier_divergence/)),
  so it leads the chain where the call's callee is a `!` (c22), for the outer of
  two nested shells (c10, ahead of c9), and with no member below the shell (c13,
  c14, the controls).
- **prettier** (its fixed point, reached on the second pass):
  - c1–c4, c6, c9, c16–c21: it places the comment in front of the member on the
    first pass (`(x?.y)/* c1 */ .p`), then pulls the block back inside the
    sealing pair (`(x?.y /* c1 */).p.t();`) — the `)`-boundary collapse
    [optional_paren_sealed_line_comment](../optional_paren_sealed_line_comment_prettier_divergence/)
    catalogs for the authored `(a?.b) /* c */.ccc`.
  - c5, c7, c15, c22: its fixed point is tsv's output; only the normalization
    differs. c10 too, while it pulls c9 inside the pair.
  - c8: same position, different break (`(() => 1)() /* c8 */⏎\t.p.t();`).
  - c11, c12: it relocates the `//` to the end of the statement
    (`(x?.y).p.t(); // c11`).

```
(x?.y) /* c1 */.p       (x?.y /* c1 */).p.t();
	.t();
```

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
