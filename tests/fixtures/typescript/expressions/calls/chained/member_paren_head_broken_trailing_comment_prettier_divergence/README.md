# A paren chain head that BREAKS, with a comment after its kept `)`

The broken-head sibling of
[member_only_paren_head_interior_line_comment](../member_only_paren_head_interior_line_comment_prettier_divergence/):
a member whose object is a parenthesized head — a cast, `typeof`, `await`, a binary, a
ternary, a unary over a chain — with a comment the author wrote after the head's `)`, where
the head's interior breaks (a `//` inside it, or width), so the `)` closes on a line of its
own:

```ts
(
	(
		x() // c1
	) as T
) // c2
	.h;
```

- **tsv**: the comment trails the `)` on the line the pair closes on — the placement it
  already has when the head fits on one line (`(x as T) // c⏎\t.h`) — and the member
  breaks below it. Across the links (`.h`, `.h()`, `[k]`, `?.h`, a longer call chain), both comment
  kinds, and the statement, assignment, declarator, `return`, `if`-test and call-argument
  positions.
- **prettier**: its first pass keeps the comment after the `)` too, and where the head
  breaks by width alone (`c38`) that placement is its fixed point. Where a `//` inside a
  retained operand shell breaks the head, prettier strips the shell
  (`(x() // c1⏎) as T` → `x() as T`, the retention tsv keeps —
  [as_satisfies_operand_line_comment](../../../as_satisfies_operand_line_comment_prettier_divergence/))
  and carries that inner comment past the `)` on its next pass, which pushes the author's
  comment off the line: own-line, then, at its fixed point two passes later, hoisted above
  the statement (`// c2⏎(⏎\tx() as T⏎) // c1⏎.h`), after a declarator's or an assignment's
  `=`, or inside `return (` / `if (` (`audit_signature.txt`). Prettier also prints the member
  at the statement's indent rather than indented under the head.

What the fixture pins is that the placement is a **fixed point**. The gap before the member
opens at the head's END, before its `)`; once the pair breaks, the `)` stands on a later line
than that anchor, and a reading anchored there alone took the comment after the `)` for an
own-line one on the next pass, moving it below (`)⏎\t// c2⏎\t.h`). A comment that follows a
`)` on its own line trails it — the gap's trailing run reads it that way, the emitter and
every predicate asked about the gap alike. A stripped shell's `)` is read the same way
(`(aaa⏎) // c27⏎.h` → `aaa // c27⏎\t.h`, prettier's placement too); the
`unformatted_ours_authored` variant carries that authoring in both comment kinds, and a
ternary head whose comment is written before the `)`; a stripped shell over a call takes the
bare call's trailing-member collapse (`(yy()⏎) // c44⏎.h` → `yy().h; // c44`). Two controls
pin the rule's edges: a
second comment after the `)` never joins a trailing `//` before it (`) // c40⏎\t\t// c41`,
no weld), and never prints ahead of an own-line comment before it (`c42` then `c43`, in
order).

The restricted productions read the same gap to decide their hanging pair, and two cases pin
where the `)` rule stops. A `)` must open its line: one behind a multi-line block's `*/`
(`yield ([p.q] /** c45⏎ */) // c46`) leaves the comment on the block's closing line once the
shell strips, which the next pass reads as own-line. A block taken after the `)`
(`throw ((…)⏎) /* c47 */ // c48`) carries the `//` glued behind it along.

Reason: Comment relocation. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
