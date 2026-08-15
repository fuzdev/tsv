# Line comment in a member gap the chain grouping would fold into a group

A call chain with a line comment in a member's gap at a position the grouping would
otherwise absorb into an existing group: after the base call with two plain members
following (`fn() // c1⏎.bar.baz`), after a member the base group takes (`fn().bar //
c5⏎.baz.qux()`), before a factory / `this` merge (`Object // c6⏎.keys(x)`), and two
such gaps in one chain.

A member printed **inside** a group can only defer its gap comment to the end of the
line (`fn().bar.baz; // c1`) — a same-line comment past two members, an own-line one
off its line, and two of them welded onto one line (`fn().bar // c2 // c3`, the second
`//` becoming text inside the first). So, following prettier's own grouping rule (a
node with a trailing comment closes its group; the merge is refused when the merged
member carries one), a member whose gap holds a line comment **starts a new group**,
and the chain-level gap emitters break around the comment and keep it where the
author wrote it.

- **tsv**: each comment in place; the members after it indent one level
  (`fn() // c1⏎\t.bar.baz;`), an initializer's chain stays on the `=` line.
- **prettier**: applies the same grouping but **relocates** the comment in three of the
  shapes — an own-line comment is hoisted before the whole statement (`// c4⏎fn().bar.baz;`),
  and a comment ahead of a refused merge is carried past the merged member (`Object.keys(x)
  // c6⏎.map(f)`, `this.x // c7⏎.y()`). Where it keeps the comment in place the difference is
  layout only: the member group at the statement's indent (`fn() // c1⏎.bar.baz;`), a break
  after `fn()` in the `.bar // c5` case, and a break after `=` in an initializer.

`unformatted_ours_glued.svelte` is the compact authoring (`fn()// c1⏎.bar.baz;`), which
tsv normalizes to `input.svelte`.

The single trailing member keeps its own rules (`trailing_member_short_chain_line_comment`,
`trailing_member_gap_comment_statement_trailer`, and the sanctioned lone-`//` collapse of
`trailing_member_after_call_comment`); this fixture is the interior-of-a-group counterpart.

Reason: Comment relocation. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
