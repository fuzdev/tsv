# curried_chain_long_leading_comment_prettier_divergence

A curried arrow chain too long for its line breaks after the `=` and stacks its heads at
one shared indent. **A comment on the `=` line, or glued to the first head, does not change
that** — the chain prints exactly as it does with no comment (`const a`, the null control),
the comment leading the first head. The width-driven sibling of
[curried_head_break_leading_comment](../curried_head_break_leading_comment_prettier_divergence/),
whose chains break by their heads' shape instead.

Prettier's answer changes with the comment's kind, and none of them is its own no-comment
layout:

- line comment — Prettier breaks after `=`, **fabricates a blank line** below the comment
  and indents the chain one level deeper
- single-line block — Prettier cancels the break after `=` (`const c = /* c */ (a…) =>`)
  and indents the remaining heads under the first
- indentable block — Prettier keeps the break and indents the whole chain one level deeper

The one kind that cannot take that answer is a **preserved multi-line block** glued to the
first head (`const f`): its own text spans lines, so inside the chain's group it would force
the break a chain that fits must not take. It leads the chain from the `=` line instead —
the first head on the comment's closing line, the rest indented under it — which is also
prettier's form, so that cell is not a divergence.

And a run the author gave **a line of its own** above the chain (`const e`) is the one gap
where prettier has a clean, stable answer — its leading own-line comment, whose chain takes
the default shape with the heads past the first indented under it. tsv matches it, so that
cell is not a divergence either; it is pinned here as the boundary of the rule.

A comment is not a layout instruction, so tsv gives one answer at every kind and at every
seam that prints through the assignment layout — a declarator, an assignment, a class
property, an object property.

`unformatted_ours_heads_inline.svelte` writes every chain on one line, the authoring that
reaches the width-driven break; `unformatted_ours_operator_own_line.svelte` puts the
assignment's `=` on a line of its own below a blank, where "the comment is on the operator's
line" has to be read off the comment's own line rather than as a distance from the target.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
