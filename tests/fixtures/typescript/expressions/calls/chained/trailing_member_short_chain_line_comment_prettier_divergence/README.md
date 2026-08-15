# Short call-chain trailing member with an own-line line comment

A chain whose base is a call and whose trailing member is not itself called
(`fn().bar` — too short for the long-chain path), with a line comment in the
gap between the call and the member.

- **tsv**: an **own-line** `//` in the gap breaks the chain at the member and
  keeps each comment where the author wrote it — a same-line comment trails the
  call (`fn() // c3`), the own-line comment keeps its own line above the member.
  This is the same break shape every longer chain already takes (see
  `trailing_member_comment`) and the member-only twin pins
  (`member_only_interior_line_comment`). A **lone same-line** `//` takes the
  sanctioned collapse instead, trailing past the member
  (`fn().bar; // c1` — see `trailing_member_after_call_comment`).
- **prettier**: hoists the own-line comment before the whole statement and keeps
  the chain inline (`// c2⏎fn().bar;`). In the mixed authoring it hoists the
  own-line comment **past** the same-line one (`// c4⏎fn() // c3⏎.bar;`),
  reversing the pair's order around the statement.

A `//` must end its line, so the gap authoring cannot stay inline without either
relocating the comment (prettier) or deferring it past the member — which
re-binds it to the statement and, with two comments, merges them onto one line
(`fn().bar; // c3 // c4`), the second `//` becoming text inside the first. tsv
breaks the chain instead, per the [comment-position
philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

`divergent_variant_gap.svelte` is prettier's stable output from the gap
authoring (the lone same-line case kept dangling, the rest hoisted); tsv
rewrites it to a third stable form (the dangling case collapses to input's,
the hoisted leading comments stay).

Uniform across `.bar`, `?.bar`, and past a non-null `fn()!`, in statement and
initializer position.

Reason: Comment relocation. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
