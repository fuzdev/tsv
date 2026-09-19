# required_pair_leading_comment_prettier_divergence

A comment the author wrote **inside** a required paren pair stays inside it. Prettier
hoists it out, in front of the whole expression the pair belongs to.

- Input: `(/* c */ b + c + d) as T` — Prettier: `/* c */ (b + c + d) as T`
- Same at every position whose pair comes from `needsParentheses`: a binary operand
  (`(/* c */ b + c + d) * 2`), a member-call base (`… .toString()`), a non-null
  operand (`…!`)
- Ours: all four keep the comment after the `(`
- A **sequence** operand's own pair is the same pair (`(/* c */ b, c) as T` — Prettier:
  `/* c */ (b, c) as T`)
- `unformatted_ours_paren_shell.svelte` doubles the shell with the comment glued to the
  inner `(` (`(/* c */ (b + c + d)) as T`) at the positions that keep the run inside —
  the cast, the non-null and `typeof`: one pair survives, and the comment is inside it.
  A binary operand and a member base hoist that authoring instead, onto the form
  `variant_hoisted.svelte` pins
- `unformatted_ours_glued_block_shell.svelte` adds the author's line breaks around the
  glued comment — ahead of it, or inside the `(` stripped from behind it. Neither is the
  comment's own, so the pair stays flat: the comment prints glued to the operand either
  way, and a pair expanded for it would fold on the next pass. The same variant rides the
  cast assignment target, the IIFE callee and the sealed optional chain fixtures

**Prettier's placement is a pipeline artifact, not a rule, and its own exception says
so.** `print/index.js` adds the `needsParentheses` parens inside `print()`, and
`ast-to-doc.js` then applies `printComments` *around* that result — so a leading
comment can only land outside a pair emitted that way. `UnaryExpression` is the one
node that prints its own pair (`estree.js`: `if (hasComment(node.argument))` →
`group(["(", indent([softline, argumentDoc]), softline, ")"])`), and there the comment
lands **inside** and prettier agrees with us — `typeof (/* c */ b + c + d)` is the null
control above. One comment, one relationship to one required pair, two placements
decided by which function emitted the parens.

The position carries authorship signal, and the two spellings are distinguishable
authorings rather than one form and its normalization: written outside the pair
(`/* c */ (b + c + d) as T`, the second null control) **both** formatters keep it
outside. Prettier collapses the two; we keep them apart.

**And with a JSDoc comment the collapse is semantic, not cosmetic.** A JSDoc cast is
`/** @type {T} */` immediately before a `(`. A comment the author wrote *inside* the pair
is therefore not a cast, and hoisting it out in front makes it one:
`const n = (/** @type {any} */ b);` is a type error tsc reports, while prettier's
`const n = /** @type {any} */ (b);` is a cast tsc accepts silently. Inert in `.ts`, where
JSDoc casts do nothing; live in any `.js` with `checkJs`. It is the mirror of prettier's
known cast-DESTROYING strip (`/** @type {A} */ (b)` → `/** @type {A} */ b`), which tsv
does not make either.

Both positions are dual-stable in our formatter — prettier's output is a fixed point of
tsv too, pinned by `variant_hoisted.svelte`, so a file already run through prettier does
not churn on the way back.

⚠️ Scope: the single-line block kind only. The **multi-line** block asks the same
position question and one more of its own — whether the comment's forced break reaches
the pair's operand — and lives in
[required_pair_multiline_leading_comment](../required_pair_multiline_leading_comment_prettier_divergence/).

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
