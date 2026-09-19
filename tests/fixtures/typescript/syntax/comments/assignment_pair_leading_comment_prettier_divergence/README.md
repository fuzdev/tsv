# assignment_pair_leading_comment_prettier_divergence

An assignment used as a value takes clarity parens (`const x = (a = b)`). A block comment
glued to that assignment prints **inside** the pair. That holds whether the author wrote
the pair (`input.svelte`) or not (`unformatted_ours_paren_less.svelte`). Prettier prints
it in front of the `(`.

- Input: `const x = (/* c */ a = b);`. Paren-less: `const x = /* c */ a = b;`. Prettier,
  from either: `const x = /* c */ (a = b);`
- The same at every position that parenthesizes an assignment: a declarator, a class
  field, an object property, a parameter or destructuring default, an arrow body, a
  `for`-of iterable, a call / array / `new` argument, a template literal expression, a
  computed key, a computed index, a statement test, a ternary branch and a `yield`
  argument. `export default` and `export =` answer the same way. An enum member's cell is
  [member_init_assignment_pair_leading_comment](../../../declarations/enum/member_init_assignment_pair_leading_comment_prettier_divergence/),
  kept apart because Svelte's compiler rejects `enum`
- Multi-line blocks likewise, and the value stays flat. Where the comment's break forces
  an enclosing list or template open, the pair still holds it (`f(⏎(/* c⏎d */ a = b)⏎)`)
- Prettier's form is dual-stable in our formatter, pinned by `variant_hoisted.svelte`:
  written in front of the pair, the comment stays there

## Reason

**◆content_preservation.** A JSDoc cast is `/** @type {T} */` immediately before a `(`.
The comment is glued to the assignment's first token, and the pair is either the
author's or one the printer adds around the assignment. Printing the comment in front of
that `(` turns a comment into a cast the author never wrote. Under `checkJs`, tsc types
`const x = /** @type {string} */ a = 1;` as `1`. Prettier's
`const x = /** @type {string} */ (a = 1);` is typed `string` and reports TS2352.
With `@type {any}` the change is silent: the value becomes `any`. Inside the pair the
comment stays a comment, which is what the author wrote. Inert in `.ts`.

The authored-inside spelling is the value-position cell of the required-pair rows
([required_pair_leading_comment](../required_pair_leading_comment_prettier_divergence/),
[required_pair_multiline_leading_comment](../required_pair_multiline_leading_comment_prettier_divergence/)).
The paren-less spelling reaches the same form. The author wrote no pair, so there is no
authored position to keep, but the placement that preserves the program is the same one.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
