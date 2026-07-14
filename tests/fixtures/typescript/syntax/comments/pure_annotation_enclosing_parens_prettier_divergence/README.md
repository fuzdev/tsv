# `@__PURE__`: enclosing parens go outside the annotation

A `/* @__PURE__ */` annotation marks the call that **follows** it as side-effect-free,
licensing a bundler (rollup, esbuild, terser) to drop the call when its result is
unused. The annotation is positional — it binds to the next expression, whatever that
is. When the formatter adds a paren around an expression whose *left edge* is an
annotated call — `??` clarity parens under a ternary, the required parens of a `??`
operand under `||`, a member/call base, a `new` callee, a `**` operand, a
value-position sequence — that paren must be emitted **outside** the comment:

```js
const a = cond ? p : (/* @__PURE__ */ f() ?? y); // tsv — `f()` is still pure
```

Prettier emits it **between** the comment and the call, so the annotation now leads a
parenthesized `??` instead:

```js
const a = cond ? p : /* @__PURE__ */ (f() ?? y); // prettier
```

`f() ?? y` is not a call, so no bundler treats it as a pure annotation — the mark is
silently inert and `f()` is retained. Nothing in the program's text was added or
removed; the annotation just moved across a boundary that changed what it binds to.
Prettier is idempotent on its own output here, so unlike the JSDoc-cast case there is
no second pass to reveal the shift.

This is the same boundary the [JSDoc cast](../jsdoc_type_cast_enclosing_parens_prettier_divergence/)
rule protects, minus the AST node: a cast is a `JsdocCast` the parser can hand
ownership of its comment to, while an annotation is an ordinary block comment glued to
an ordinary call. tsv therefore holds *any* leading comment against the token it
precedes rather than enumerating bundler vocabularies — an annotation set is
open-ended (`@__PURE__`, `#__PURE__`, `@__NO_SIDE_EFFECTS__`, `@__KEY__`, …) and every
term tsv failed to know would be a silent loss.

`const m` is the boundary: with no enclosing paren to add, the annotation already sits
against its call and tsv and prettier agree.

See [conformance_prettier.md §Comment relocation](../../../../../../docs/conformance_prettier.md#comment-relocation).

## Contexts tested

Ternary alternate / consequent / test (clarity parens around `??`); `??` mixed with
`||`; member base; callee; `new` callee; `**` operand; value-position sequence; the
`#__PURE__` spelling; an annotated `new`. `unformatted_ours_clarity.svelte` authors
the ternary operands *without* the optional clarity parens (the same AST) — tsv adds
them around the annotation, prettier adds them after it.

## Related fixtures

- [jsdoc_type_cast_enclosing_parens](../jsdoc_type_cast_enclosing_parens_prettier_divergence/) — the same paren boundary for a JSDoc cast, where the comment is owned by an AST node
- [nullish_branch](../../../expressions/ternary/nullish_branch/) — the `??` clarity parens themselves, comment-free
- [test_paren_leading_comment](../../../expressions/ternary/test_paren_leading_comment_prettier_divergence/) — an ordinary (non-annotation) glued comment in the same stripped-paren position, preserved the same way (every glued block comment is owned)
