# Divergence: line comment between index-signature `]` and the value `:`

A line comment in the `]`→`:` gap (`[k: string] // c\n: number`).

Prettier **relocates** the comment into the brackets, trailing the key type, and
breaks the bracket; tsv keeps the comment after `]` where the author wrote it and
drops the value `:` to the next line, **indented one level** so the continuation
reads as part of this member (uniform forced-continuation indent).

```ts
// prettier (relocates into brackets)   // tsv (preserves placement)
[                                        [k: string] // c
	k: string // c                       	: number;
]: number;
```

The **own-line** authoring (`[k: string]⏎// c⏎: number`) pulls up to trail `]`
and reaches input under tsv — one pass (`unformatted_ours_own_line.svelte`).
Prettier gets there in two: its first pass pulls the comment up but keeps the
value `:` flush (`[k: string] // c⏎: number`, the
`prettier_intermediate_to_variant_own_line.svelte` form), its second relocates
into the brackets — `variant_own_line.svelte`, the same form as
`output_prettier.svelte`, dual-stable.

A **block** comment in this gap (`[k: string] /* c */ : number`) stays inline in
both formatters and is not a divergence — only a line comment differs (it runs to
EOL, so the value `:` must drop to its own line; otherwise it would swallow
`: number` — content loss); the own-line **block** authoring diverges too and is
the sibling
[index_signature_bracket_colon_own_line_block_comment](../index_signature_bracket_colon_own_line_block_comment_prettier_divergence/).
Same preserve-comment-position rule as elsewhere —
see [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Comment Position Philosophy.
