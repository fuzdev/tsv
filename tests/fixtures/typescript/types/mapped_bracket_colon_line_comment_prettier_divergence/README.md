# mapped_bracket_colon_line_comment_prettier_divergence

A line comment in a mapped type's `]`→value-`:` gap
(`[K in keyof T] // c⏎: V;`). tsv keeps the comment after `]` and drops the
value `:` to a continuation line indented one level (uniform
forced-continuation indent) — the same layout as its index-signature sibling
([index_signature_bracket_colon_line_comment](../type_members/index_signature_bracket_colon_line_comment_prettier_divergence/)).
Emitting the `: V` inline would let the `//` swallow it — content loss.

Prettier instead breaks the brackets and relocates the comment inside them,
trailing the key constraint on its own indented line
(`[⏎K in keyof T // c⏎]: V;`, one pass — `output_prettier.svelte`).

The own-line authoring (`[K in keyof T]⏎// c⏎: V;`) pulls up to the same
continuation form under tsv in one pass. Prettier reaches its in-bracket
landing from that authoring non-idempotently: pass 1 crosses the `:` and
hangs the value (`[K in keyof T]: // c⏎V;`), pass 2 moves the comment inside
the brackets. That landing is prettier-stable but **not** tsv-stable: tsv
rewrites it to its own bracket-break layout, keeping `K in keyof T` on the `[`
line (`[K in keyof T // c⏎]: V;` — the
[mapped_key_line_comment](../mapped_key_line_comment_prettier_divergence/)
fixed point), a third stable form — so the landing is pinned as
`divergent_variant_own_line.svelte` with prettier's unstable first pass as
`prettier_intermediate_to_divergent_variant_own_line.svelte`.

`unformatted_ours_compact.svelte` is the glued spelling (`]// c⏎:V`): tsv
normalizes it to input; prettier lands on its in-bracket form.

The same-gap **block** comment is
[mapped_bracket_colon_comment](../mapped_bracket_colon_comment_prettier_divergence/);
two line comments in this gap leave prettier with no fixed point at all
([mapped_bracket_colon_multi_comment](../mapped_bracket_colon_multi_comment_prettier_divergence/)).

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation and §Uniform Forced-Continuation Indent.
