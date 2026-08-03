# computed_key_bracket_colon_own_line_block_comment_prettier_divergence

An own-line **block** comment in a computed member key's `]`→`:` gap
(`[x]⏎/* c */⏎: number;`), in an interface and a type literal. A single-line
block forces nothing, and a comment in this gap trails the `]` (a trailing
position), so tsv collapses the authored breaks and keeps the comment inline in
its authored syntactic slot (`[x] /* c */: number;` — the form its inline
sibling [computed_key_bracket_comment](../computed_key_bracket_comment_prettier_divergence/)
pins) — one pass, runs in order and each comment distinct.

Prettier **relocates** the comment into the brackets, trailing the key
expression (`[x /* c */]: number;`), from **both** authorings — the inline
authoring in one pass (`output_prettier.svelte`, the same move the inline
sibling pins), the own-line authoring non-idempotently: pass 1 crosses the `:`
and hangs the value (`[x]: /* c */⏎number;`), pass 2 moves the comment inside
the brackets — one comment per pass, so an own-line run takes 3 passes (two
distinct intermediates, past what a `prettier_intermediate_*` pin carries; the
run is therefore left inline in `unformatted_ours_own_line.svelte` and its
chain is recorded here). The relocated landing is dual-stable — tsv preserves a
comment *authored* inside the brackets — so the two authorings keep two fixed
points, and only prettier moves between them.

- `unformatted_ours_own_line.svelte` — the own-line authoring (single case):
  tsv normalizes it to input; prettier's chain lands on the in-bracket
  relocation instead.
- `prettier_intermediate_to_variant_own_line.svelte` — prettier's unstable
  first pass on that variant.
- `variant_own_line.svelte` — the in-bracket landing, dual-stable.
- `unformatted_ours_compact.svelte` — the glued spelling (`[x]/* c */:number`):
  tsv normalizes it to input; prettier lands in-bracket.

The same-shape gap at the other computed-key hosts: object literal
([computed_key_bracket_colon_own_line_block_comment](../../../expressions/objects/computed_key_bracket_colon_own_line_block_comment_prettier_divergence/)),
class `]`→`=`
([computed_key_bracket_own_line_block_comment](../../../statements/class/computed_key_bracket_own_line_block_comment_prettier_divergence/)),
destructuring pattern
([computed_key_bracket_colon_own_line_block_comment](../../../expressions/destructuring/computed_key_bracket_colon_own_line_block_comment_prettier_divergence/))
— prettier hangs the value at those hosts, where here it converges on the
in-bracket relocation like its index-signature and mapped-type neighbours
([index_signature_bracket_colon_own_line_block_comment](../index_signature_bracket_colon_own_line_block_comment_prettier_divergence/),
[mapped_bracket_colon_comment](../../mapped_bracket_colon_comment_prettier_divergence/)).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment relocation.
