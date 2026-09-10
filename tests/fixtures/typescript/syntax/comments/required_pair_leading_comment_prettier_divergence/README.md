# required_pair_leading_comment_prettier_divergence

A comment the author wrote **inside** a required paren pair stays inside it. Prettier
hoists it out, in front of the whole expression the pair belongs to.

- Input: `(/* c */ b + c + d) as T` — Prettier: `/* c */ (b + c + d) as T`
- Same at every position whose pair comes from `needsParentheses`: a binary operand
  (`(/* c */ b + c + d) * 2`), a member-call base (`… .toString()`), a non-null
  operand (`…!`)
- Ours: all four keep the comment after the `(`

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

Both positions are dual-stable in our formatter — prettier's output is a fixed point of
tsv too, pinned by `variant_hoisted.svelte`, so a file already run through prettier does
not churn on the way back.

⚠️ Scope: the single-line block kind only. A **multi-line** block in the same position
also makes tsv break the pair's operand where prettier keeps it flat
(`(/* c⏎d */ b +⏎c +⏎d) as T`) — a separate defect of the same break-propagation class,
not sanctioned here, and deliberately kept out of this fixture so nothing pins that
break as intended.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
