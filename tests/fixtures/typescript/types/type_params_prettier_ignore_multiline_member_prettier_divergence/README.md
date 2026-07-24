# type_params_prettier_ignore_multiline_member_prettier_divergence

A `/* prettier-ignore */` glued to a type parameter whose frozen slice spans
multiple lines (a multi-line object constraint kept verbatim).

**tsv** expands the `<…>` one parameter per line — its layout whenever a member
spans lines — so the frozen slice sits on its own line between the angle
brackets. Both hosts behave identically: the width-decided declaration path
(`function fn1<…>`) and the always-inline method-signature path
(`class C { fn2<…>() }`, shared by interface-member and call/construct
signatures):

```ts
function fn1<
	/* prettier-ignore */ T extends {
	x:   1
}
>(a: T): void {}
```

**Prettier** keeps the `<…>` flat, gluing the brackets around the verbatim
slice (`output_prettier.svelte`) — its printed-ignored slice is a plain string
doc, invisible to its break propagation:

```ts
function fn1</* prettier-ignore */ T extends {
	x:   1
}>(a: T): void {}
```

## Reason

Expanding the list when a frozen member is multi-line keeps the frozen slice
visually delimited and the parameters uniformly presented, rather than gluing
the angle brackets onto the lines of an opaque verbatim block. A single-line
frozen parameter keeps the inline layout (matching prettier — see the ordinary
`type_params_prettier_ignore_member` fixture). The type-parameter analog of
`union_prettier_ignore_multiline_member_prettier_divergence` and
`tuple_prettier_ignore_multiline_member_prettier_divergence`.

See [conformance_prettier.md §Format-ignore directive](../../../../../docs/conformance_prettier.md#format-ignore-directive).
