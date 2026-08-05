# Divergence: nested-composite member shell normalizes in one pass

The nested-composite face of
[union_intersection_parens_line_comment](../union_intersection_parens_line_comment/)'s
sanctioned strip: the redundant shell ends an **intersection inside a union member**
(`B & (A // c1⏎) | C`), directly or through further redundant layers. The `|` that
follows licenses the strip — the union's per-member break ends the line where the shell
ends, so the deferred comment flushes in the union's member gap, trailing the member.
Both formatters agree on the fixed point:

```ts
type A1 =
	| (B & A) // c1
	| C;
```

The divergence is the **path** there, not the form. Prettier's first pass also breaks
the intermediate intersection — a break the reparse cannot reproduce, since the comment
now sits in the union's member gap and the intersection is comment-free — so prettier
needs **two passes** (`| (B &⏎↹↹↹A) // c1` first, pinned in
`prettier_intermediate_flat.svelte`). tsv emits the fixed point in one pass: the
deferred comment forces only the group it actually flushes in (the union), and the
intersection it escapes prints flat.

`unformatted_ours_flat.svelte` carries the flat authorings, which reach `input` in one
pass under tsv only.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
