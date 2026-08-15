# Short call-chain trailing member: a same-line gap comment with a comment behind it

A chain whose base is a call and whose trailing member is not itself called
(`fn().bar`), with a **same-line** line comment in the call→member gap and **another
comment behind it in that same gap**.

The lone same-line `//` takes the sanctioned collapse (`fn() // c⏎.bar;` →
`fn().bar; // c` — see `trailing_member_after_call_comment`): the comment defers to the
end of the line, and with nothing else there that is lossless. The licence stops where
its argument stops. `trailing_member_gap_comment_statement_trailer` pins one boundary —
a comment trailing the *statement*, which would weld with the deferred `//`. This is the
other: a comment inside the **same gap**, which the flat path emits *inline* while the
`//` ahead of it is deferred, so the run comes out **reordered** (`fn() /* c2 */.bar;
// c1` — c2 has moved ahead of c1) and the block loses its authored line.

Whether the follower is a block or a line comment does not change the argument, and the
line-comment spelling is the same rule read through its own arm: an own-line `//` behind
a same-line one forces the chain open (`trailing_member_short_chain_line_comment`). Both
boundaries are one sentence — the deferral is licensed only while nothing else reaches
that line end.

- **tsv**: breaks the chain, each comment where the author wrote it — the shape the
  line-comment spelling and every longer chain already take.
- **prettier**: keeps the pair distinct but hoists the own-line comment before the whole
  statement (`/* c2 */⏎fn() // c1⏎.bar;`), its usual relocation, and breaks after `=` in
  an initializer — the same relocation
  [trailing_member_short_chain_line_comment](../trailing_member_short_chain_line_comment_prettier_divergence/)
  pins for the line-comment spelling.

The follower may also be **owned** — a block glued to the property (`/* c16 */.bar`) is
printed by the member's own doc rather than by this gap. Ownership is a fact about who
PRINTS a comment, never about whether it occupies the page, so the gate asks the
**on-page** axis: an owned follower lands ahead of the deferred `//` exactly like an
emitted one. `unformatted_ours_glued.svelte` is that authoring; tsv normalizes it to
`input.svelte` (the chain-level gap emitter puts a space after the `*/`).

Two controls hold the boundary from the other side: a lone same-line `//` still collapses
(`fn().bar; // c12`), and a gap of blocks only stays inline (`fn() /* c13 */ /* c14
*/.bar;`) — nothing defers there, so nothing can be reordered.

Uniform across `.bar`, `?.bar`, past a non-null `fn()!`, in statement and initializer
position, with and without a same-line block ahead of the `//`.

Reason: Comment relocation. See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
