# curried_head_break_leading_comment_prettier_divergence

A curried arrow chain whose heads force the break (`arrow_chain_should_break`) breaks
after the `=` and stacks its heads under it. A **leading comment in the `=`→value gap
does not move that break** — the chain prints exactly as it does with no comment, the
comment leading the first head. Prettier's answer instead changes with the comment's
kind:

- single-line block — Prettier: `const c = /* x */ ({}) =>` (no break after `=`)
- indentable block — Prettier breaks after `=` **and** indents the chain one level deeper
- line comment — Prettier breaks after `=` and **fabricates a blank line** below it
- no comment — Prettier breaks after `=`, and we match (the null control)

Three answers to one question, one of them inserting a blank line the author did not
write, and the no-comment control showing that prettier's own rule for this chain is the
break we keep. A comment is not a layout instruction, so tsv gives one answer at every
kind.

A **preserved multi-line block** (`a`) is the boundary of the rule, and there tsv takes
prettier's form: `const a = /* x⏎y */ ({}) =>`, the chain led from the `=` line with no break
after it. The comment's own lines are the break, and it is the answer a chain that breaks for
width gets too ([curried_chain_long_leading_comment](../curried_chain_long_leading_comment_prettier_divergence/)),
so the two break causes agree — at every assignment seam
([curried_head_break_preserved_block_comment](../curried_head_break_preserved_block_comment/)).
The other boundary is a run the author gave **lines of its own** above the chain
(`const a =⏎/* x */⏎({}) =>`): prettier's leading own-line comment, whose default chain shape
tsv matches ([curried_head_break_own_line_comment](../curried_head_break_own_line_comment/)).

The comment's placement — with the value, on the line below the `=` — is not a choice
this arm makes either: the comment is glued to the first head's `(`, so the arrow OWNS it
and prints it from its own doc. The arm's job is only to not lose it.

⚠️ **The last two cells carry a DROP this arm had, and they are the load-bearing files.**
The arm printed only what the value OWNED and nothing else, so any comment in the gap the
value did not own was lost outright (`docs/comments.md` hazard 1). Two authorings reach
that:

- a **comment RUN** (`const f = /* x */ /* y */ ({}) => () => test;`) — only the run's
  LAST comment is glued to the head's `(`, so only it is owned; `/* x */` was dropped;
- a **redundant paren shell** (`unformatted_ours_paren_shell.svelte`, which writes every
  cell as `const a = /* x⏎y */ (({}) => () => test);`) — the shell strips and the comment
  is owned by the node the strip discards, so the value owns *nothing* and the whole run
  went. This is the same bug at its extreme, not a second one.

Every block kind was affected; a `//` was not, which is why no formatted corpus and no
fixed-point gate could see it — the wrong output is its own fixed point and reparses. The
shell variant also states the rule that closes it: a redundant shell strips and changes
nothing else, so the shell authoring reaches the bare authoring's fixed point.

`unformatted_ours_head_on_equals.svelte` is prettier's own output, kept as a normalization
test: tsv turns it back into `input.svelte` in ONE pass. So this is not a dual-stable pair —
the two positions are not both fixed points of tsv — and the claim is the stronger one, that a
file already formatted by prettier converges here instead of churning. `output_prettier.svelte`
alone could not say that: it is an oracle claim about prettier and carries no tsv claim at all.

See [conformance_prettier.md](../../../../../../docs/conformance_prettier.md) §Comment Position Philosophy and
[conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
