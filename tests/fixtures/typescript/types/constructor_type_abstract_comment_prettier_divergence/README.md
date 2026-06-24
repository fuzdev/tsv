# Constructor type `abstract` keyword comment divergence

Prettier relocates a comment between the `abstract` modifier and the `new`
keyword of a constructor type, mirroring how it relocates a `new`-to-params
comment ([constructor_type_new_comment](../constructor_type_new_comment_prettier_divergence/)):

- No params: `abstract /* c */ new ()` → `abstract new () /* c */ => T`
- With params: `abstract /* c */ new (x: T)` → `abstract new (/* c */ x: T) => T`
- With type params: `abstract /* c */ new <T>()` → `abstract new /* c */ <T>() => T`
- Both gaps at once: `abstract /* a */ new /* b */ ()` → `abstract new () /* a */ /* b */ => T` (the `abstract`→`new` and `new`→`(` gaps relocate independently; we keep both in place)

We preserve the comment in the user's original position between `abstract` and
`new`. Per comment placement policy, user intent is preserved when prettier moves
comments to different syntactic positions. Without preservation the comment was
dropped entirely (content loss).

Both positions are dual-stable under our formatter (`variant_after_parens.svelte`).

See [conformance_prettier.md §Comment relocation](../../../../../docs/conformance_prettier.md#comment-relocation).
