# JSDoc type cast: enclosing parens go outside the cast comment

A JSDoc cast is the comment **plus** the `(` it is glued to: `/** @type {A} */ (b)`
casts `b`. When the formatter adds a paren around an expression whose *left edge* is
a cast — `??` clarity parens under a ternary, the required parens of a `??` operand
under `||`, a member/call base, a `new` callee, a `**` operand, a value-position
sequence or assignment — that paren must be emitted **outside** the comment:

```js
const a = cond ? x : (/** @type {A} */ (b).c ?? d); // tsv — cast still on `b`
```

Prettier emits it **inside**, between the comment and the cast's own `(`:

```js
const a = cond ? x : /** @type {A} */ ((b).c ?? d); // prettier, pass 1
const a = cond ? x : /** @type {A} */ (b.c ?? d); // prettier, pass 2 (fixed point)
```

This is a `◆prettier_bug` on two counts. Prettier's pass-1 output is **not a fixed
point** — reparsed, the comment is now glued to the *enclosing* paren, so that paren
becomes the cast and the original `(b)` decays to a redundant grouping paren, which
pass 2 strips (see `output_prettier.svelte` and the pinned chain in
`audit_signature.txt`). And the fixed point it lands on has **changed the program's
meaning**: `@type` annotated `b`, and now annotates the whole `b.c ?? d` expression,
so `b` — the value whose type the author was asserting — goes unchecked. tsv keeps
the cast on the node the author wrote it on, and reaches a fixed point in one pass.

tsv holds the pair together structurally rather than by re-deriving it from text: the
parser gives the `JsdocCast` node **ownership** of its comment, so the comment is
printed by the cast, never by an enclosing gap, and a synthesized paren cannot land
between the two. Prettier re-decides cast-hood from text adjacency on every parse,
which is exactly why its own output doesn't survive a second pass.

`const n` is the boundary: with no enclosing paren to add, the comment already sits
against its own `(` and tsv and prettier agree.

See [conformance_prettier.md §JSDoc / paren semantics](../../../../../../docs/conformance_prettier.md#jsdoc--paren-semantics).

## Contexts tested

Ternary alternate / consequent / test (clarity parens around `??`); `??` mixed with
`||`; member base; callee; `new` callee; `**` operand; value-position sequence and
assignment. `unformatted_ours_clarity.svelte` authors the three ternary operands
*without* the optional clarity parens (the same AST) — tsv adds them around the
comment, prettier adds them after it.

## Related fixtures

- [jsdoc_type_cast_extent](../jsdoc_type_cast_extent/) — `(a.b)` vs `(a).b` cast extent, the distinction this fixture protects
- [jsdoc_type_cast_svelte](../jsdoc_type_cast_svelte/) — JS `<script>` casts prettier preserves (tsv matches)
- [jsdoc_type_cast_nested](../jsdoc_type_cast_nested/) — nested casts, each level keeping its parens
- [nullish_branch](../../../expressions/ternary/nullish_branch/) — the `??` clarity parens themselves, cast-free
- [test_paren_leading_comment](../../../expressions/ternary/test_paren_leading_comment/) — a *non*-cast comment in the same stripped-paren position (no gluing: the comment leads the parens)
