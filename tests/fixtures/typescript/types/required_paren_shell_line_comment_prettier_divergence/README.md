# required_paren_shell_line_comment_prettier_divergence

A redundant paren shell whose **trailing** gap holds a **line** comment, at the seven
positions where the shell and a pair the enclosing construct **requires** are the *same*
pair: a prefix type-operator operand (`keyof` / `readonly`), an optional tuple element, a
conditional type's check and `extends` positions, an array element, an indexed-access
object, and a type-parameter `extends` constraint. Every operand here is one its construct
needs parens for (a conditional, a union), which is what makes the coincidence reachable at
all.

**tsv**: retains the shell and opens it, the comment keeping the line it was written on:

```
type A =
	keyof (
		B extends C ? D : E // c
	);
```

**Prettier**: strips the shell, re-adds the required pair, and carries the comment out to
the end of the enclosing line — past the `;` (`type A = keyof (B extends C ? D : E); // c`),
past the `?` (`[(J extends K ? L : M)? // c]`), past the `extends` operand
(`(O extends P ? Q : R) extends S // c`).

Retention is the rule every bracketed type region already follows for its own trailing
gap, and the argument is losslessness rather than taste: a `//` carried past its own
construct lands on a line that may already hold one, where the two render back to back and
the second becomes text of the first — irreversibly, the merged form being a fixed point in
both formatters. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## What the coincidence adds: a pair COUNT

Placement is the entry above's; what these positions add is that the comment must render
inside that one pair and never mint a second. tsv emitted
`keyof ((⏎↹B extends C ? D : E // c⏎))` — and the same doubling at all five sibling
positions — until every site that adds a required pair was routed through one seam
(`Printer::build_required_paren_operand_doc`), which asks the single retain/strip predicate
(`Printer::paren_retains_for_trailing_run`) the shell's own emission already asks. That is the enclosing-side reading of the rule the
redundant-paren-member entry states: *a comment never changes which parens are retained,
only where it renders once they are.*

## Cases

- **A** — prefix operator, conditional operand. The divergence.
- **F** — prefix operator, **union** operand: **prettier agrees here**, keeping its own
  pair open around the comment. The control that shows the retention is not a blanket tsv
  preference — the two formatters part only where prettier's own answer is inconsistent.
- **I** — optional tuple element.
- **N**, **V** — the conditional's check and `extends` positions.
- **D1** — array element, where the `[]` suffix rides *outside* the pair decision: the
  shell may already print the parens, but the suffix is the array type's own and always
  emits.
- **I1** — indexed-access object, hugging the `=` like **D1**'s array twin above: the
  retained shell breaks internally, so the alias keeps its head on the `=` line rather than
  also breaking. The enclosing `=`'s gate asks the seam that decides the pair
  (`Printer::required_paren_pair_opens`), so the two positions cannot answer one question
  two ways.
- **Z2** — the same pair carrying a **leading** run as well. Two of the seven positions
  have a keyword→value hang seam in front of the pair
  (`Printer::keyword_value_stripped_paren_hang`), and a leading `//` must not take it —
  that strips the very pair the operand needs, re-adds it bare, and lifts the trailing
  `//` out past the `;` (`keyof // c1⏎(A3 extends B3 ? C3 : D3); // c2`, prettier's form),
  where a second trailing comment then welds onto it. This is the prefix operator's; the
  constraint's is below. The other five have no hang seam and already retained.
- **fn**, **E2** — the type-parameter `extends` constraint and its `infer` twin, the
  second position with a keyword→value hang seam in front of the pair. The hang stripped
  the shell here too, and the constraint's arm had to re-mint the pair — which for the
  `infer` case is not a layout choice: without it the enclosing conditional's `?` rebinds
  and the canonical parser rejects the output. Retaining also fixes an idempotence hole
  the lift opened: an own-line comment in the shell's trailing gap was lifted through a
  `hardline` one indent level deeper than the value→`>` gap where the next pass finds that
  same comment, so tsv's output was not a fixed point.
- **O1**, **T1**, **Y1** — the comment-free controls, pinning the pair count a comment
  must not change.
