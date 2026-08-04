# mapped_bracket_colon_comment_prettier_divergence

A block comment in a mapped type's `]`→value-`:` gap
(`[K in keyof T] /* c */ : V;`). A comment in this gap trails the `]` (a
trailing position), so tsv keeps it inline in its authored syntactic slot,
before the `:` — the same spaced form its index-signature sibling holds stable
([index_signature_bracket_colon_comment](../type_members/index_signature_bracket_colon_comment/))
— runs in order and each comment distinct.

Prettier instead **relocates** the comment into the brackets, trailing the key
constraint (`[K in keyof T /* c */]: V;`), in one pass from this authoring —
so unlike the index-signature sibling, the inline authoring is itself a
divergence (`output_prettier.svelte`).

The own-line authoring (`[K in keyof T]⏎/* c */⏎: V;`) collapses to the same
inline form under tsv in one pass — a single-line block forces nothing, and
own-line-ness is authoring signal for a leading position, not a trailing one.
Prettier reaches its in-bracket relocation from that authoring
non-idempotently: pass 1 crosses the `:` and hangs the value
(`[K in keyof T]: /* c */⏎V;`), pass 2 moves the comment inside the brackets —
one comment per pass, so the own-line run takes 3 passes (two distinct
intermediates, past what a `prettier_intermediate_*` pin carries; the run is
therefore left inline in `unformatted_ours_own_line.svelte` and its chain is
recorded here). The relocated landing is dual-stable — tsv preserves a comment
*authored* inside the brackets — so the two authorings keep two fixed points,
and only prettier moves between them.

- `unformatted_ours_own_line.svelte` — the own-line authoring (single case):
  tsv normalizes it to input; prettier's chain lands on the in-bracket
  relocation instead.
- `prettier_intermediate_to_variant_own_line.svelte` — prettier's unstable
  first pass on that variant.
- `variant_own_line.svelte` — the in-bracket landing, dual-stable.
- `unformatted_ours_compact.svelte` — the glued spelling (`]/* c */:V`): tsv
  normalizes it to input; prettier lands in-bracket.

The same-gap **line** comment is
[mapped_bracket_colon_line_comment](../mapped_bracket_colon_line_comment_prettier_divergence/);
the optional-marker gaps are
[mapped_optional_marker_comment](../mapped_optional_marker_comment_prettier_divergence/);
a comment *authored* trailing the key constraint inside the brackets is a plain
match ([mapped_bracket_comment](../mapped_bracket_comment/)).

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
