# Constructor type `new` keyword comment divergence

Prettier relocates comments between the `new` keyword and the type parameters /
opening `(` of a constructor type:

- No params: `new /* c */ ()` → `new () /* c */ => T`
- With params: `new /* c */ (x: T)` → `new (/* c */ x: T) => T`
- With type params: `new /* c */ <T>()` → `new /* c */ <T>() => T` (kept in place)

We preserve the comment in the user's original position between `new` and the
params. Per comment placement policy, user intent is preserved when prettier
moves comments to different syntactic positions. Without preservation the comment
was dropped entirely (content loss).

Both positions are dual-stable under our formatter (`variant_after_parens.svelte`).

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
