# yield_operand_prettier_ignore_head_prettier_divergence

`yield` / `yield*` share the restricted-production freeze with `return` / `throw` — an
own-line directive in the grouping `(`→operand gap freezes the operand whole — and tsv keeps
the hanging-paren layout the own-line comment forces:

```ts
yield (
	// prettier-ignore
	a  +  b
);
```

Prettier relocates the directive onto the **keyword's** line and strips the grouping parens
(`yield // prettier-ignore⏎a  +  b;`) — the pre-existing `yield` comment relocation, cataloged
at [§Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation)
([yield_open_paren_line_comment](../../../syntax/comments/yield_open_paren_line_comment_prettier_divergence/)),
here carrying the directive with it. The freeze rides on that divergence; it is not a second
one, and the `return` / `throw` siblings match prettier exactly
([operand_prettier_ignore_head](../operand_prettier_ignore_head/)).

Prettier's relocated form is **not a fixed point**: its next pass reformats the plain-`yield`
operand (`a + b`), losing the freeze, while the `yield*` one survives —
`audit_signature.txt` pins the chain. That is the concrete cost of relocating a directive, and
why tsv never does.

## Reason

A directive's placement is what decides whether it is honored, so an emitter must not move it.
See
[conformance_prettier.md §Format-ignore directive](../../../../../../docs/conformance_prettier.md#format-ignore-directive).
