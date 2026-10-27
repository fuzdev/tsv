# Left-spine shell comment through nested discarded pairs: convergence

A block comment glued inside a grouping pair on a value's left spine, nested inside
further pairs on that spine: `x = ((/*c*/ (a) ** 2) ** 2) ** 2;`, a `+`/`*`
chain under a member tail (`x = ((/*c*/ (a + 2) * 2) * 2).b`), and a conditional operand
(`x = ((/*c*/ (a ? b : c) ** 2) ** 2) ** 2`). The comment leads the whole value, ahead of
every pair the spine prints. Both formatters reach the **same fixed point** — `input` — and
differ on the **first** pass.

- **tsv**: one pass. The comment moves out past every discarded pair to the value's head
  (`x = /*c*/ ((a ** 2) ** 2) ** 2;`).
- **prettier**: moves it out one pair per pass (`x = (/*c*/ (a ** 2) ** 2) ** 2;`), and
  reaches `input` within three passes (`audit_signature_nested.txt` pins the chain).

The same holds below the pair a keyword-like seam prints its operand in — a spread
(`x = [...((/*c*/ (a) || d) && d)]` prints `x = [.../*c*/ ((a || d) && d)]`), `await` and a `new`
callee — where the comment leads the operand from the seam's own gap, ahead of that pair. A
prefix unary prints it in its own operator→operand gap too, inside the comment-holder pair
the comment earns and ahead of the operand's pair (`x = !((/* d */⏎ a.b) + 1).c` prints
`x = !(/* d */ (a.b + 1).c)`).
The same holds for a block glued to a leaf the reparse would hand it to (`x = ((/*c*/ (a) + 2) + 2).b`
prints `x = /*c*/ (a + 2 + 2).b`) and for a sequence's envelope (`x = ((/*c*/ (a)).b, 1)` prints
`x = /*c*/ (a.b, 1)`), where prettier's first pass leaves the comment just inside the pair.
A shell the member chain widens a member over is the exception: its comment prints mid-chain,
after the chain's first link, in both formatters' fixed point
(`` x = ((/*c*/ (a).b).m() + 1)`t` `` prints `` x = (a /*c*/.b⏎.m() + 1)`t` ``).

At `return` the same block whose line breaks only with the chain is glued to a space as well,
so near the print width the comment rides the chain's first line inside the binary's pair —
`return ((/* d */⏎a).b ** zzz…) ** q` at the width where the chain still fits on that line
(100 columns) and one past it, where the chain breaks inside the pair. Left as a line inside the
pair's group, it broke with the chain, and the reparse printed a second form.

At `yield` the claimed run leads the argument, ahead of the pair the binary prints: where it
keeps a line of its own (`yield ((/* d */⏎⏎a).b ** zz) ** q`) the argument takes the
restricted production's hanging pair, and where its line breaks only with the chain the
binary's conditional pair, which prints with that break. These are the cases where the two
formatters do NOT meet: prettier prints `yield /* d */⏎⏎(a.b ** zz) ** q` from `input`
(`output_prettier.svelte`, `audit_signature.txt`), and the line break after `yield` is ASI —
the reparse is a bare `yield` followed by an expression statement.

The second `yield` case's 83 `z`s sit past a window that is a known gap: with 68–80 (67–80
at `yield*`, at this fixture's depth) the comment's line breaks with the chain on the first pass and tsv's second pass
prints a different form, so that authoring is not yet its own fixed point (a TODO at
`build_yield_doc`; the glue `return` and `throw` take there measured worse at `yield`).

`unformatted_ours_nested` carries the authoring.

See
[conformance_prettier_ts.md §TypeScript](../../../../../../docs/conformance_prettier_ts.md#typescript)
(Left-spine shell comment through nested discarded pairs, convergence) and
[conformance_prettier.md §Authoring Convergence Philosophy](../../../../../../docs/conformance_prettier.md#authoring-convergence-philosophy).
