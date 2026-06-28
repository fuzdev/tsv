# Member-only chain with interior line comments

A member-only chain (pure property access, **no calls** — `foo.bar.baz`) with a
line comment in a gap between members.

- **tsv**: breaks the chain at every member and keeps each comment where the
  author wrote it — a same-line comment trails its member (`.bar // c1`), an
  own-line comment stays on its own line before the next member (`// c2` above
  `.baz`). This is the same break shape a call in the chain already forces (see
  `trailing_member_comment`), now applied to call-free chains too.
- **prettier 3.9**: hoists the own-line comment onto its own line after `=`
  (`const a =⏎\t// c2⏎\tfoo`), keeps the same-line comment trailing its member
  (`.bar // c1`), and partially breaks the chain. A single own-line comment
  (case b) is hoisted after `=` and the chain stays inline
  (`const b =⏎\t// c1⏎\tfoo.bar.baz;`).

A `//` must end its line, so a member-only chain with an interior line comment
cannot stay inline without either relocating the comment (prettier) or fusing it
into the line below (the historical tsv bug: `foo.bar.baz; // c2 // c1`). tsv
breaks the chain instead, applying the [comment-position
philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
— comments stay where the author placed them. Block-only comments don't force
this (they format inline on the fill path), so only line comments route here.

Reason: Comment relocation. See
[conformance_prettier.md §Comment relocation](../../../../../../../docs/conformance_prettier.md#comment-relocation).
