# export_equals_prettier_ignore_head_prettier_divergence

`export =` carries the same value head as
[`export default`](../default_declaration_prettier_ignore_head_prettier_divergence/): an own-line
directive in the `=`→value gap freezes the value, and the keyword stays outside the slice.

```ts
export =
	// prettier-ignore
	aaa  +  bbb;
```

Prettier **relocates** the directive flush onto the `export =` line and freezes anyway; tsv keeps
the author's line, since a keyword-trailing directive is inert under the placement floor and the
relocated form would lose the freeze on the second pass. `unformatted_ours_spaces.svelte` pins
the freeze's scope: a perturbed `export   =` sits outside the slice and normalizes.

The gap one delimiter earlier — between `export` and `=` — is a different position: a `=` begins
no node, so Rule A has nothing to bind to there and a directive freezes nothing. It cannot reach
this seam either, since the freeze window opens only past the `=`.

## Reason

Rule A binds a directive to the node that follows it, and the placement floor makes a
keyword-trailing directive inert; ◆comment_preservation. See
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
