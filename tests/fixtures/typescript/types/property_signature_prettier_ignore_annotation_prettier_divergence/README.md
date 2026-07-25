# property_signature_prettier_ignore_annotation_prettier_divergence

An own-line directive in the gap **before** a property signature's `:` freezes the
whole `: type` annotation — and tsv keeps the directive **own-line**, where the
author put it:

```ts
interface A {
	a
		// prettier-ignore
		: {x:   1};
}
```

Covers the interface member, the type-literal member, and both sides of an optional
`?` marker: a directive **after** it (with the block spelling, which the placement rule
treats identically) freezes the `: type` alone, while one **before** it starts the
freeze at the marker — the text the directive precedes, `?: {w:   4}` as a unit.

Prettier re-binds the directive **past the `:`** — it pulls the `:` back onto the key
and freezes the *type* alone (`a: // prettier-ignore⏎{x:   1}`), a scope tsv reads
from the directive's position instead: it freezes the construct it precedes, which
here is the whole annotation. Where a second comment already sits in the gap prettier
also **merges the two onto one line** (`b: // prettier-ignore // c`), which makes the
second `//` ordinary text — a content loss tsv's own-line placement avoids. Prettier's
form is not self-stable either: its second pass floats the directive past the `;`
(`a: { x: 1 }; // prettier-ignore`) and loses the freeze, and moves the block spelling
in front of the `?` (non-idempotent, pinned via `audit_signature.txt`).

`unformatted_ours_spaces.svelte` perturbs whitespace outside the frozen slices; tsv
normalizes it to input (prettier does not — it relocates the directive).

The sibling gap — a directive **after** the `:`, freezing the type alone — is
[annotation_prettier_ignore_own_line](../annotation_prettier_ignore_own_line_prettier_divergence/);
the same before-`:` rule at the other heads is
[binding](../binding_prettier_ignore_annotation_prettier_divergence/),
[index signature](../index_signature_prettier_ignore_annotation_prettier_divergence/),
and [return type](../return_type_prettier_ignore_annotation_prettier_divergence/).

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
