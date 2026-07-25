# index_signature_prettier_ignore_annotation_prettier_divergence

An own-line directive in the gap between an index signature's `]` and its value `:`
freezes the whole `: type` annotation — and tsv keeps the directive **own-line**,
where the author put it:

```ts
interface A {
	[k: string]
		// prettier-ignore
		: {x:   1};
}
```

Both hosts (interface member and type-literal member) take the same emitter.

Prettier freezes the same span but **relocates** the directive to trail the `]`
(`[k: string] // prettier-ignore`) and dedents the frozen slice
(`output_prettier.svelte`). tsv cannot adopt that form: a head-trailing directive is
**inert** under tsv's placement classification, so the relocated form would lose the
freeze on the second pass — the authored own-line placement is the idempotent fixed
point (and the comment-position doctrine). Prettier's relocated form is not
self-stable either: its own second pass breaks the bracket, pulls the directive
*inside* it to trail the key type, and loses the freeze (non-idempotent, pinned via
`audit_signature.txt`).

`unformatted_ours_spaces.svelte` perturbs whitespace outside the frozen slices; tsv
normalizes it to input (prettier does not — it relocates the directive).

⚠️ A **second** comment in this gap makes prettier oscillate forever between the two
placements, with no fixed point at all — the plain-comment case is pinned by
[index_signature_bracket_colon_multi_comment](../type_members/index_signature_bracket_colon_multi_comment_prettier_divergence/),
so this fixture keeps the single-directive shape (where prettier does converge) and
the multi-comment interaction rides the other before-`:` fixtures
([binding](../binding_prettier_ignore_annotation_prettier_divergence/),
[property signature](../property_signature_prettier_ignore_annotation_prettier_divergence/),
[return type](../return_type_prettier_ignore_annotation_prettier_divergence/)).

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
