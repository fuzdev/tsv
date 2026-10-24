# glued_block_paren_line_comment_prettier_divergence

A block comment glued to a redundant grouping `(` — nothing but spaces between its `*/` and the
paren — binds to the parenthesized value, exactly as it binds to a bare value it is glued to.
When the pair's interior opens with a `//`, stripping the pair puts that `//` between the block
and the value: the block ends up behind a line comment, or alone at the end of its line, and no
longer leads anything. So tsv keeps the pair, and the block stays glued to the `(` it was
written against:

```ts
// tsv                               // prettier
const v7 = /* @__PURE__ */ (         const v7 /* @__PURE__ */ =
	// side-effect free                  // side-effect free
	fn()                                 fn();
);
```

A `/* @__PURE__ */` annotation is where this is visible to a tool: the annotation marks the
call it leads, and in the stripped forms it leads nothing, so a bundler is free to stop
honoring it (rollup does, on the `kw /* @__PURE__ */ // d⏎g()` form). There is no content
sniff, as for every glued block comment: a plain `/* c */` binds the same way.

Every expression position takes the same pair: `await`, a `new` callee, a `case` test,
`return` / `throw` / `yield`, `void`, a declarator, an assignment, a class field, an enum
member, an object property, an arrow body, a call argument, an array element, a template
interpolation, a parameter and a destructuring default, a binary operand, a ternary branch, a
statement-level expression, a `for`-`of` iterable, a spread, and a pair that is only the left
edge of the value (`/* c24 */ (…).b()`, `/* c25 */ (…) + b`). It is the layout a JSDoc cast
already takes for the same interior (`/** @type {T} */ (⏎// d⏎x⏎)`). Under `void` the
operator's own comment-holding pair wraps it, as it wraps a cast
(`void (⏎/* c7 */ (⏎// d7⏎a⏎)⏎)`). Prettier strips the pair at every position and places the
block where its own attachment puts it: behind the keyword with the `//` on the same line,
ahead of the `=` / `:` / `?` / `+`, inside the parens a `return` / `void` re-emits, on one
line with the `//` at an arrow body, a call argument, an array element, a template
interpolation, a spread and a statement-level expression, or — at `yield` — after a `yield;` it
inserts on its second pass, which drops the argument.

The block may be the tail of a glued run (`/* a31 */ /* c31 */ (`) or span lines (`c32`); it
is the one glued to the `(` that binds. The run inside the pair keeps its authoring: a blank
between two of its comments, or between the last one and the value, survives (`c27`, `c28`,
`c29`), and an own-line block above the `//` keeps its line (`c29`, `c30`), as in every pair
tsv keeps.

A `//` the author glued to the kept `(` keeps that line (`/* g1 */ ( // h1`), the
opening-delimiter rule, as at `return (` and a JSDoc cast's `(`: the block at the end of the
script repeats each cell written that way (`g`/`h` for `c`/`d`) but `c29` and `c30`,
whose run opens with an own-line block.

## Variants

- `unformatted_ours_paren_blank.svelte` — a blank between the `(` and the run; it sits against
  the delimiter and is erased, as at every bracket.
- `unformatted_ours_nested_paren.svelte` — a second redundant pair inside the first
  (`/* c1 */ ((⏎// d1⏎a))`); the pairs strip as a unit and one is printed.
- `unformatted_ours_nested_paren_blank.svelte` — `c27` written as two nested pairs, each with
  a blank after its `(`: the outer blank is erased, and the inner one — above an own-line
  comment, inside a stripped shell — survives between the two comments, as the
  condition-shell entry of §Comment relocation keeps it
  ([condition_shell_blank_before_comment](../../../statements/if/condition_shell_blank_before_comment_prettier_divergence/)).

## Reason

**Comment position** — a glued block comment binds to the value it leads, and a stripped pair
must not let another comment land between them. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
(a block comment glued to a grouping paren whose interior opens with a line comment).

## Related fixtures

- [glued_block_paren_no_line_comment](../glued_block_paren_no_line_comment/) — the controls: an
  interior holding only same-line blocks, or nothing, strips in both formatters and the block
  stays glued to the value
- [default_value_glued_block_paren_line_comment](../../../modules/exports/default_value_glued_block_paren_line_comment_prettier_divergence/) — `export default` and `export =`
- [expr_glued_block_paren_line_comment](../../../../svelte/syntax/comments/expr_glued_block_paren_line_comment_prettier_divergence/) — Svelte template expressions
- [pure_annotation_enclosing_parens](../pure_annotation_enclosing_parens_prettier_divergence/) — the mirror rule: a paren the printer ADDS never lands between a glued comment and its token
- [keyword_value_paren_blank_comment](../keyword_value_paren_blank_comment_prettier_divergence/) — the type-position twin, where a type owns no comment and the pair still strips
