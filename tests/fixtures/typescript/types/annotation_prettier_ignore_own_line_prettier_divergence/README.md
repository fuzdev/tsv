# annotation_prettier_ignore_own_line_prettier_divergence

An own-line directive between a type annotation's `:` and its type freezes the type
that follows — and tsv keeps the directive **own-line**, where the author put it:

```ts
let v:
	// prettier-ignore
	{x:   1};
```

Prettier freezes the same span but **relocates** the directive to trail the `:` on the
head's line (`let v: // prettier-ignore`) and drops the frozen slice to the head's
indent (`output_prettier.svelte`). tsv cannot adopt that form: a head-trailing
directive is **inert** under tsv's placement classification (see
[union_prettier_ignore_trailing_annotation_head](../union_prettier_ignore_trailing_annotation_head_prettier_divergence/)),
so prettier's relocated form would lose the freeze on the second pass — keeping the
authored own-line placement is both the comment-position doctrine and the idempotent
fixed point. Prettier's relocated form is not even self-stable: at the
property-signature host its **own** second pass floats the directive to the end of the
member and loses the freeze (`a: { x: 1 }; // prettier-ignore` — non-idempotent,
pinned via `audit_signature.txt`). `unformatted_ours_spaces.svelte` perturbs
whitespace outside the frozen slices; tsv normalizes it to input (prettier does not —
it relocates the directives).

Hosts covered: variable annotation, function return type, interface property
signature, and a multi-line frozen type (kept verbatim).

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
