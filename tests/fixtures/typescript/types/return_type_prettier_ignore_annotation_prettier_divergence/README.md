# return_type_prettier_ignore_annotation_prettier_divergence

An own-line directive in the gap between a signature's `)` and its return-type `:`
freezes the whole `: type` annotation — and tsv keeps the directive **own-line**,
where the author put it:

```ts
function fn()
	// prettier-ignore
	: {x:   1} {
	return { x: 1 };
}
```

Covers every host of the shared `)`→`:` emitter: function declaration, class method,
arrow function, and the type-member signatures (method + call, the latter with the
block spelling, which the placement rule treats identically).

Prettier freezes the same span but **relocates** the directive to trail the `)`
(`function fn() // prettier-ignore`) and dedents the frozen slice
(`output_prettier.svelte`); where the params carry a comment it also breaks them open.
tsv cannot adopt that form: a head-trailing directive is **inert** under tsv's
placement classification, so the relocated form would lose the freeze on the second
pass — the authored own-line placement is the idempotent fixed point (and the
comment-position doctrine). Prettier's relocated form is not self-stable either: its
chain takes three passes to settle, dropping the directive into the function **body**
(`{⏎// prettier-ignore⏎return …`) and back inside the params, losing every freeze but
the arrow's (non-idempotent, pinned via `audit_signature.txt`).

`unformatted_ours_spaces.svelte` perturbs whitespace outside the frozen slices; tsv
normalizes it to input (prettier does not — it relocates the directive).

The `)`→`=>` twin — a function *type*'s return annotation — is
[function_type_prettier_ignore_return](../function_type_prettier_ignore_return_prettier_divergence/);
the sibling gap after the `:` is
[annotation_prettier_ignore_own_line](../annotation_prettier_ignore_own_line_prettier_divergence/).

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
