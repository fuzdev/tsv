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

A **block** comment in this gap (`[k: string] /* c */ : number`) stays inline in
both formatters and is not a divergence — only a line comment differs (it runs to
EOL, so the value `:` must drop to its own line; otherwise it would swallow
`: number` — content loss). Same preserve-comment-position rule as elsewhere —
see [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation and §Comment Position Philosophy.
