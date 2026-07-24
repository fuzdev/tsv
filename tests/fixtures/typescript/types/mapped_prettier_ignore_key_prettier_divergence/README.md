# mapped_prettier_ignore_key_prettier_divergence

A directive **inside a mapped type's bracket** — own-line or glued before the
`K in ...` binding — freezes only the **binding**; the value type keeps formatting
normally:

```ts
type A = {
	[
		// prettier-ignore
		K   in   keyof T
	]: V;
};
```

Prettier's mapped-type handler freezes the **whole mapped type** instead — including
the `]: V` value side and the `[` that *precedes* the directive
(`prettier_variant_frozen.svelte` keeps `]:   V;` verbatim; it is prettier-stable and
tsv normalizes it to input). tsv freezes only what the directive precedes.
`unformatted_ours_value_spaces.svelte` perturbs the value side (and structural
whitespace); tsv normalizes it to input, prettier does not.

A directive **above** the whole `[K in ...]: V` clause (between `{` and `[`) freezes
the whole clause in both tools — the ordinary
[mapped_prettier_ignore_signature](../mapped_prettier_ignore_signature/) fixture.

## Reason

The freeze scope follows the directive's position: a directive freezes the construct
it **precedes**. Freezing the whole mapped type from inside the bracket would freeze
content *before* the directive. Prettier's whole-node redirect is a special-cased
handler tsv deliberately does not copy.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
