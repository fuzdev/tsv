# Short call-chain trailing member: same-line gap comment behind a later comment in its statement

A chain whose base is a call and whose trailing member is not itself called
(`fn().bar`), with a **same-line** line comment in the call→member gap **and** a later
comment in the chain's statement, or a trailer just past it, that the collapsed line would
carry.

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

The comment can arrive from anywhere the collapsed statement would print on the
member's line, so the read takes **every** comment between the chain's end and the end
of its statement, behind whatever tokens stand between — and past the statement's end, a
trailer read **through closing punctuation** (`)` `]` `}` `;` `,`), because the
expanded layout's own reprint puts `);` on a line below the member; a same-line read
would collapse it back on pass two. That reach is what the array-`]`, object-`}` and
block-`}` cases pin.

The cases from `// c19` on are the ones only the statement-wide read sees, each nesting
the short chain inside something that keeps printing after it: a lookup's brackets and
the call the lookup is the callee of (`obj[fn() // c19⏎.bar](); /* c20 */` — collapsed,
`obj[fn().bar](); /* c20 */ // c19`, reordered), the same in an initializer, an arrow
body with a lookup on the arrow — including `/* c32 */`, a comment inside the statement
ahead of a chain that continues past it — an operator's right side, and a following
argument. Every comment counts there, whatever its kind and wherever its line: one
behind further tokens moves with the layout (the expanded reprint gives a block leading
the next argument a line of its own), so a read that asked for a trailer would expand on
the first pass and collapse on the second.

The block-`}` case is the rule firing **conservatively**: a block's `}` always takes a
line of its own, so `// c18` could never actually land on the deferred comment's line,
and the expansion is merely unneeded there. Deciding otherwise needs a layout fact no
build-time read has, and the conservative answer is never *wrong* — pinning it keeps the
boundary visible. The statement-wide read is conservative the same way (a comment past a
broken list's own line), and its reach is the whole statement: a chain in a class field
expands on a comment anywhere later in the class declaration, and a chain in a large object
literal on one anywhere later in that object's statement. Each such expansion is
prettier's own shape and loses nothing. A nested chain with nothing after it in its statement still
takes the collapse — see `trailing_member_after_call_comment`.

`unformatted_ours_collapsed.svelte` is the glued authoring (`fn()// c1⏎.bar; // c2`);
`unformatted_ours_dot_gap.svelte` puts the comment after the `.` (`fn().// c1⏎bar;`),
which tsv moves ahead of the dot as it does for a called member. Both normalize to
`input.svelte` under tsv.

Uniform across `.bar`, `?.bar`, past a non-null `fn()!`, in statement, initializer,
call-argument, array-element, object-value, block-tail, lookup-bracket, arrow-body and
binary-operand position, and for a `//` or `/* */` trailer.

Reason: stable-quirk normalization at the boundary of the after-call collapse. See
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
