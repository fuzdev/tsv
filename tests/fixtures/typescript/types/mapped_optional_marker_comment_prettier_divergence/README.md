# mapped_optional_marker_comment_prettier_divergence

A block comment in a mapped type's optional-marker gaps: before the marker
(`[K in keyof T] /* c */?: V;`) and between the marker and the value `:`
(`[K in keyof T]? /* c */ : V;`, likewise `+?`/`-?`).

tsv keeps each comment on its authored side of the marker — the
property-signature parity: a comment before the `?` stays before it
(`a /* c */?: A`, a match in both formatters there), a comment after the
marker trails it, spaced before the `:`
([optional_marker_comment](../../syntax/comments/optional_marker_line_comment_prettier_divergence/)'s
block siblings; the mapped `]`→`:` gap without a marker is
[mapped_bracket_colon_comment](../mapped_bracket_colon_comment_prettier_divergence/)).

Prettier relocates every one of these into the brackets, trailing the key
constraint (`[K in keyof T /* c */]?: V;`), in one pass — the same in-bracket
destination as its `]`→value-`:` relocation, erasing which side of the marker
the author put the comment on. The relocated landing is dual-stable, so the
authored and relocated positions keep two fixed points and only prettier moves
between them.

`unformatted_ours_compact.svelte` is the glued spelling (`]?/* c */:V`): tsv
normalizes it to input; prettier lands in-bracket.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
