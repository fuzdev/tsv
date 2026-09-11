# function_type_prettier_ignore_return_prettier_divergence

An own-line directive between a function type's `=>` and its return type freezes the
return type — and tsv keeps the directive **own-line**, where the author put it:

```ts
type A = () =>
	// prettier-ignore
	{x:   1};
```

Prettier freezes the same span but **relocates** the directive to trail the `=>`
(`() => // prettier-ignore`), dropping the frozen slice to the head's indent. tsv
cannot adopt that form: an arrow-trailing directive is **inert** under tsv's
placement classification, so prettier's relocated form would lose the freeze on tsv's
second pass — keeping the authored own-line placement is both the comment-position
doctrine and the idempotent fixed point.

The `type D` and `type E` controls pin that a **plain** comment keeps whichever line
the author gave it: trailing the `=>` (`D`) it stays there, and on its own line (`E`)
it keeps that line. This gap is a keyword→value gap, where both placements are stable,
so the directive changes only *whether the return type freezes*, never the placement
or the layout below it — the return type hangs one level in either way. Prettier
leaves `D` in place with the type flush (the indent-only divergence at
[Fn/ctor-type `=>`→return-type line comment](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation))
and pulls `E` up to trail the `=>` (the relocation cataloged beside it,
[function_type_return_own_line_line_comment](../function_type_return_own_line_line_comment_prettier_divergence/)).
`unformatted_ours_own_line_control.svelte` authors both controls flush below their
first line; tsv hangs each to input's form without moving either comment.

`type F` moves the directive one gap earlier — between the `)` and the `=>` — and
spells it as a **block** comment. Placement, not spelling, keys honoring, so the
directive still freezes; the frozen span there is the whole `=> T` annotation (the
node the directive precedes), and the directive keeps its own line above it:

```ts
type F = ()
/* prettier-ignore */
=> {x:   4};
```

Prettier honors the block spelling at that position too — it relocates the directive
to trail the `)` and keeps the same frozen slice. That relocated form is the one part
of `output_prettier.svelte` that is **not** self-stable: prettier's second pass
rejoins `=> {x:   4}` onto the `)` line (freeze intact), which
`audit_signature.txt` pins. The `type A`–`E` forms are self-stable.

Hosts covered: function type, constructor type (`new () =>`), and abstract
constructor type (`abstract new () =>`) — the same return-type position on each,
plus the pre-arrow `)` gap on the function type.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
