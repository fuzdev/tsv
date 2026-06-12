# union_infix_pipe_line_comment_prettier_divergence

Line comment trailing the infix `|` of a union (`A | // c\n B`).

**Prettier**: relocates the comment to trail the previous member:
```
| A // c
| B
```

**tsv**: keeps the comment on the separator/`B` side, on its own line so the
pipe stays attached to the member (`variant_trailing.svelte` is prettier's form):
```
| A
// c
| B
```

Per Comment Position Philosophy: the comment sits after the `|` separator, so
tsv associates it with the `B` side rather than relocating it to trail `A`.
Both positions are dual-stable; the divergence is in normalization —
`unformatted_ours_infix.svelte` normalizes to input under tsv, to
`variant_trailing.svelte` under prettier.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment relocation.
