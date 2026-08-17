# paren_shell_glued_leading_line_comment_prettier_divergence

A **line** comment the author wrote on a retained paren shell's `(` line (`( // c`). tsv
keeps it there; **prettier moves it to its own line** inside the shell — or, where it also
strips the shell, out of the construct entirely.

```
// tsv                     // prettier
type A = [                 type A = [
	( // c1                     (
		B | C                       // c1
	)?                            B | C
];                            )?
                           ];
```

## Reason: it is the opening-delimiter rule, and `(` was the last holdout

What the author put on an opening delimiter's line stays on it, and what they put on its own
line keeps its own line — so both authorings are fixed points. tsv already answered this way
at every other delimiter, and diverges from prettier at every one of them: `fn( // c`,
`[ // c`, `{ // c`, `Array< // c`, `function f( // c` all keep the comment where it was
written, while prettier drops it to the next line in all five. The paren shell was the one
place tsv followed prettier instead, so a `(` answered the question differently from the
`[`, `{` and `<` beside it. The emitter behind it
(`Printer::push_paren_shell_leading_run`) decided its separator from the comment's *kind*
rather than from what follows it — the last leading-run site in the printer that did.

An author **blank line** below the comment rides along under the same rule: which line the
comment sits on and how far the author separated it from the type are two different facts,
and claiming the `(` line for the comment must not cost the blank underneath it. Prettier
keeps that blank too, at its own un-glued placement. A blank between the `(` and the comment
is a different question and stays erased — it sits against the delimiter, where tsv and
prettier drop it at every bracket alike.

⚠️ The blank half is **not** the family's shared answer. The list and call families
(`fn(`, `[`, `{`) deliberately DROP it, on the reading that a pulled comment leaves the blank
in the container's own leading gap — which both formatters discard when no comment sits there
at all; the `return (` / `throw (` / `yield (` hang and the ASI operand shell drop it too, by
omission rather than by that argument. Both readings are pinned, in opposite directions, and
the split is unresolved — it is the `DelimiterGluedBlank` axis; see
[comments.md §The delimiter-line question](../../../../../docs/comments.md).

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **c1** — optional tuple element, **union** operand. The shape prettier renders open, which
  is what made the paren the holdout: this one position agreeing was taken as the rule.
- **c2** — the same element with an operand the pair is genuinely required for. Prettier
  hoists the comment out in front of the pair here rather than into it, so it answers one
  gap two ways keyed on the operand's kind.
- **c3–c4** — a redundant shell retained by its own trailing run; prettier strips it and
  carries both comments out.
- **c5** — union member. The member's own indent is the union printer's `align(2)` offset,
  unchanged by the glue and identical in the comment's own-line authoring.
- **c6–c7** — prefix type-operator operand.
- **c8–c9** — the author blank below the glued comment. Prettier erases the blank *and*
  welds the two comments onto one line (`as S; // c8 // c9`), losing one of them.
- **c11**, **c12** — array element and indexed-access object, the two required-pair
  positions that reached the pair through their own bare `(`…`)` rather than through the
  shell's opener. Neither asked the glue question because neither asked the *open* question
  at all: the shell's stripped arm emitted the run and the operand, and the caller's pair
  closed around the result — `(// c⏎N extends O ? P : Q)[]`, the `(` glued with no space,
  the operand never indented into the shell, and the `)` welded to its tail. A fixed point
  that reparses, so only a `compare` ever saw it.
- **c10**, **c13** — the controls: written on its own line it keeps its own line, and at
  **c10 prettier agrees** — the two formatters part on preserving the author's choice, not
  on one layout. `c13` is the own-line half at one of the two positions above, which had no
  own-line form to keep.
