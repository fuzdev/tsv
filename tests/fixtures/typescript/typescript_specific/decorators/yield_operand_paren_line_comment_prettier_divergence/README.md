# yield_operand_paren_line_comment_prettier_divergence

A `//` comment in the grouping `(`→operand gap of a plain `yield` whose operand is a decorated
class. The comment takes the hanging pair every restricted production takes
([yield_open_paren_line_comment](../../../syntax/comments/yield_open_paren_line_comment_prettier_divergence/)),
and that pair is also the one the class needs after `yield`
([yield_operand_paren](../yield_operand_paren_prettier_divergence/)), so tsv prints one pair, not
two (`yield ( // c⏎\t(⏎\t\t@dec…⏎\t)⏎)` would be a second fixed point one shell deeper):

```ts
yield ( // c
	@dec
	class {}
);
```

Prettier trails the comment on the keyword and strips the pair, from either authoring:

```ts
yield // c
@dec
class {};
```

## Reason

◆prettier_bug. `yield` is `yield [no LineTerminator here] AssignmentExpression`, so ASI ends
the `yield` at the newline and `@dec class {}` becomes a statement of its own, where a
decorated class expression cannot stand: prettier throws on its own pass 1. tsv keeps the
author's pair, which keeps both the break and the `@` legal. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(`yield`/`yield*` grouping `(`→argument line comment) and
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

## Cases

- the comment authored on the `(` line;
- the comment authored on its own line below the `(`.

Both are tsv fixed points; prettier gives both the same throwing form, so no
`audit_signature.txt` is pinned (prettier's chain stops at its pass 1).
