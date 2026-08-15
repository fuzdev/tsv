# Short call-chain trailing member: same-line gap comment behind a statement trailer

A chain whose base is a call and whose trailing member is not itself called
(`fn().bar`), with a **same-line** line comment in the call→member gap **and** a
comment trailing the statement (or the enclosing call) on the member's line.

The lone same-line `//` takes the sanctioned collapse (`fn() // c⏎.bar;` →
`fn().bar; // c` — see `trailing_member_after_call_comment`): the comment defers to
the end of the line, and with nothing else there that is lossless. That licence
stops exactly where its argument stops. With a trailer, the statement's own `// c2`
flushes at the same line end and the two **weld** — `fn().bar; // c1 // c2`, the
second `//` becoming text inside the first — and a block trailer is **reordered**
behind the deferred one (`fn().bar; /* c8 */ // c7`). So the chain breaks at the
member instead, each comment where the author wrote it — the same shape a called
member (`fn() // c⏎\t.bar(); // c1`) and every longer chain already take.

- **tsv**: breaks the chain, indenting the continuation member one level and keeping
  an initializer's chain on the `=` line (`const a = fn() // c3⏎\t.bar; // c4`).
- **prettier**: keeps each comment in place too, but prints the member at the
  statement's indent (`fn() // c1⏎.bar; // c2`) and breaks after `=` in an
  initializer (`const a =⏎\tfn() // c3⏎\t.bar; // c4`) — a layout-only difference.

The trailer is read from the chain's source end **through closing punctuation**
(`)` `]` `}` `;` `,`), because the expanded layout's own reprint puts `);` on a line
below the member — a same-line read would collapse it back on pass two. That reach is
what the array-`]`, object-`}` and block-`}` cases pin. The last of them is the rule
firing **conservatively**: a block's `}` always takes a line of its own, so `// c18`
could never actually land on the deferred comment's line, and the expansion is merely
unneeded there. Deciding otherwise needs a layout fact no build-time read has, and the
conservative answer is never *wrong* — pinning it keeps the boundary visible.

`unformatted_ours_collapsed.svelte` is the glued authoring (`fn()// c1⏎.bar; // c2`);
`unformatted_ours_dot_gap.svelte` puts the comment after the `.` (`fn().// c1⏎bar;`),
which tsv moves ahead of the dot as it does for a called member. Both normalize to
`input.svelte` under tsv.

Uniform across `.bar`, `?.bar`, past a non-null `fn()!`, in statement, initializer,
call-argument, array-element, object-value and block-tail position, and for a `//` or
`/* */` trailer.

Reason: stable-quirk normalization at the boundary of the after-call collapse. See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
