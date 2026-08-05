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

The `type D` control pins that a **plain** comment keeps the trailing relocation
(`() => // c`): only a directive earns own-line preservation. What follows the
comment takes the same continuation indent the directive cases above take — one
level in — so the freeze changes *which* placement is kept, never the layout
below it; prettier leaves that continuation flush, the divergence cataloged at
[Fn/ctor-type `=>`→return-type line comment](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
`unformatted_ours_own_line_control.svelte` authors that control comment own-line;
tsv normalizes it to input's trailing form.

`type E` moves the directive one gap earlier — between the `)` and the `=>` — and
spells it as a **block** comment. Placement, not spelling, keys honoring, so the
directive still freezes; the frozen span there is the whole `=> T` annotation (the
node the directive precedes), and the directive keeps its own line above it:

```ts
type E = ()
/* prettier-ignore */
=> {x:   4};
```

Prettier honors the block spelling at that position too — it relocates the directive
to trail the `)` and keeps the same frozen slice. That relocated form is the one part
of `output_prettier.svelte` that is **not** self-stable: prettier's second pass
rejoins `=> {x:   4}` onto the `)` line (freeze intact), which
`audit_signature.txt` pins. The `type A`–`D` forms are self-stable.

Hosts covered: function type, constructor type (`new () =>`), and abstract
constructor type (`abstract new () =>`) — the same return-type position on each,
plus the pre-arrow `)` gap on the function type.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
