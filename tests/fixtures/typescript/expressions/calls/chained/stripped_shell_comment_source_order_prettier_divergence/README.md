# Comment in a stripped shell around a member's object, kept in authored order

A comment in grouping parens the parser strips from around a member's object, in a
member chain a call roots (`(// c5⏎x /* c6 */.b).g()`). The
`unformatted_ours_stripped_shell` variant holds those authorings; `input.svelte` is where
tsv prints them, in one pass.

- **tsv**: the comment prints in front of the member the shell wrapped (the mid-chain
  stable quirk, c9: `x.y /* c9 */.p`) only while no other comment would print between the
  shell's run and that member. Where the object holds a comment too (c1–c4), or the
  member's own gap does (c5–c8), it leads the chain instead, in authored order — where the
  spelling without the parens puts it (`// c5⏎x /* c6 */.b`). A comment inside a computed
  lookup's brackets prints after the run either way, so the run still prints in front of
  the lookup (c10).
- **prettier**: prints it in front of the member regardless, after everything the object
  prints, and buckets the member gap's comments by kind, so the comment the author wrote
  first prints after the others (`/* c4 */ q /* c3 */.p`, `x /* c6 */.b // c5`, and c1 on
  a line of its own before `.h`). Every line of `input.svelte` is a prettier fixed point,
  so the divergence is the normalization alone (`audit_signature_stripped_shell.txt`).

```
(// c5         →  tsv:      // c5
x /* c6 */.b).g();          x /* c6 */.b
                            	.g();
                  prettier: x /* c6 */.b // c5
                            	.g();
```

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [conformance_prettier.md §Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
