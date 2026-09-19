# member_init_assignment_pair_leading_comment_prettier_divergence

An enum member's assignment initializer prints inside clarity parens (`A = (a = b)`), the
same pair a declarator, a binding default, an object property and an arrow body print. A
comment the author wrote **inside** that pair stays inside it. Prettier hoists it in front.

- Input: `A = (/* c */ a = b)`. Prettier: `A = /* c */ (a = b)`
- Multi-line: `A = (/* c⏎d */ a = b)`. Prettier: `A = /* c⏎d */ (a = b)`. The value stays
  flat in both formatters
- Null control (`E3`): written outside the pair, both formatters keep it there

This is the enum member's cell of the value-seam rows in
[required_pair_leading_comment](../../../syntax/comments/required_pair_leading_comment_prettier_divergence/)
and
[required_pair_multiline_leading_comment](../../../syntax/comments/required_pair_multiline_leading_comment_prettier_divergence/).
It lives here because Svelte's compiler rejects a TypeScript `enum`. Those fixtures are
`svelte compile`-analyzable, and an enum would demote their render-equivalence check to
the template-only fallback.

Prettier's form is dual-stable in our formatter, pinned by `variant_hoisted.svelte`.

## Reason

**Comment position.** Prettier's `needsParentheses` pair is added inside `print()` and
`printComments` wraps the result, so a leading comment can only land outside a pair emitted
that way. The two spellings are distinct authorings: with a JSDoc comment, prettier's
relocated form is a cast under `checkJs` where the authored one is not.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
