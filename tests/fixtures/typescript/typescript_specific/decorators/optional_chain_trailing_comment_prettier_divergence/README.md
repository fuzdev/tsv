# decorator optional-chain trailing comment - prettier divergence

A comment authored between a decorator expression and the closing paren of its shell,
where the expression is an **optional chain** (`@(a?.b /* c */)`).

## Why tsv Differs

tsv prints a decorator's trailing comment run **outside** the shell, so both authorings
converge on one fixed point:

```typescript
@(a?.b /* c */)   // authored inside
@(a?.b) /* c */   // authored outside — and what tsv prints for both
```

The parens are the **printer's**, not the author's: `build_decorator_expression_doc`
adds or omits them from the expression's *shape* (a bare `DecoratorMemberExpression`
with at most one call needs none), never from how the source spelled it. A position
defined relative to a paren tsv chose to synthesize carries no authorial signal, so
there is nothing to preserve there — and placing the run outside means a `//` needs no
forced-open shell, since the decorator's own trailing break already ends the line.

**Prettier agrees on every other expression shape** and prints the comment outside:
`@(a + b) /* c */`, `@(a[b]) /* c */`, `@(x!) /* c */`, `@(a ? b : c) /* c */`, and
`@fn() /* c */` (parens dropped entirely). Those are the match cases, pinned by
[expression_trailing_comment](../expression_trailing_comment/).

The optional chain is prettier's lone exception, and an **artifact rather than a rule**:
`a?.b` parses to a `ChainExpression` wrapping the `MemberExpression`, and the comment
attaches to the inner node, so prettier prints it within the chain's own output. Two
tells that no rule is being expressed — prettier is **stable on both spellings** here
(`@(a?.b) /* c */` is left untouched, which is why the inside form is pinned as a
`prettier_variant_*` rather than as `output_prettier`), and the same chain with a **line**
comment goes back outside (`@(a?.b // c` → `@(a?.b) // c`), matching tsv. A block comment
on a chain is the only cell in the grid that differs.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Expected behavior

- **tsv**: `@(a?.b) /* c */` from either authoring — one fixed point
- **prettier**: keeps whichever spelling it is given; `prettier_variant_inside_parens.svelte`
  is the second fixed point tsv normalizes away
