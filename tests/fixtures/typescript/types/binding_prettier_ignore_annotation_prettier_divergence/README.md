# binding_prettier_ignore_annotation_prettier_divergence

An own-line directive in the gap **before** a binding's `:` freezes the whole
`: type` annotation — and tsv keeps the directive **own-line**, where the author put
it:

```ts
class A {
	a
		// prettier-ignore
		: {x:   1} = { x: 1 };
}
```

Covers every host of the shared before-`:` emitter: class property, function
parameter, variable binding, and index-signature key. When an optional `?` marker sits
between the key and the `:`, the freeze starts at the **marker** — the text the
directive precedes, marker included — so `?: {w:   4}` is frozen as a unit. (Its
definite `!` counterpart is unreachable in this shape: TypeScript's grammar forbids a
line break before `!`, so no directive can be alone on its line above one.)

Prettier freezes the same span but **relocates** the directive to trail the binding
(`a // prettier-ignore`) and dedents the frozen slice (`output_prettier.svelte`). tsv
cannot adopt that form: a head-trailing directive is **inert** under tsv's placement
classification, so the relocated form would lose the freeze on the second pass — the
authored own-line placement is the idempotent fixed point (and the comment-position
doctrine). Prettier's relocated form is not self-stable either: its own second pass
drags the class property's directive onto the initializer `=` and loses the freeze
(non-idempotent, pinned via `audit_signature.txt`).

`unformatted_ours_spaces.svelte` perturbs whitespace outside the frozen slices; tsv
normalizes it to input (prettier does not — it relocates the directive).

The sibling gap — a directive **after** the `:`, freezing the type alone — is
[annotation_prettier_ignore_own_line](../annotation_prettier_ignore_own_line_prettier_divergence/);
a glued directive is inert
([type_heads_prettier_ignore_glued_inert](../type_heads_prettier_ignore_glued_inert_prettier_divergence/)).

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
