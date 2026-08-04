# conditional_prettier_ignore_branch_prettier_divergence

An own-line directive above a conditional type's `? {…}` (resp. `: {…}`) branch line
freezes that branch's child — and tsv keeps the directive **own-line**, where the
author put it:

```ts
type A = X extends string
	// prettier-ignore
	? {x:   1}
	: { y: 2 };
```

Prettier freezes the same child but **relocates** the directive to trail the `?`
(resp. `:`) on the branch line (`? // prettier-ignore`), indenting the frozen slice
below (`output_prettier.svelte`, self-stable). tsv cannot adopt that form: a
directive sharing its line with the `?`/`:` token is **inert** under tsv's placement
classification, so prettier's relocated form would lose the freeze on tsv's second
pass — keeping the authored own-line placement is both the comment-position doctrine
and the idempotent fixed point.

A **composite** branch (`type C`) freezes **whole**, operators and all — the
interposing `?` / `:` token means the union's own leading-run walk can never reach
the directive, so the member rules that apply at a bare single-child head (first
member freezes) have nothing to bind to here; the branch the directive precedes is
the freeze target. Prettier agrees on that scope (both members stay verbatim in
`output_prettier.svelte`) and diverges only in the placement.

The `type D` control pins that a **plain** comment keeps the trailing relocation
(`X extends string // c` — a fixed point for both tools): only a directive earns
own-line preservation. (The relocation itself — an own-line plain comment moving to
its trailing fixed point — is pinned by the
[fn/ctor return](../function_type_prettier_ignore_return_prettier_divergence/)
fixture's `unformatted_ours_own_line_control.svelte`; here prettier's plain-comment
relocation target differs from tsv's, so that variant shape can't make a
prettier-side claim.)

Branches covered: true (`?`) and false (`:`) — the same placement rule on each.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
